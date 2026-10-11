//! 工具注册表与调用守卫。
//!
//! `execute` 的执行顺序(每一步失败都返回结构化结果,由上层包装成 `ok=false` 回填给模型):
//! 白名单 → 查找 → 危险工具开关 → 参数 schema 校验 → 带超时执行。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::tools::builtin;

/// 工具来源:内置(本地执行)还是调用方声明(需要把结果回填)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    Builtin,
    Client,
}

/// 暴露给模型/客户端的工具描述。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema(参数)
    pub parameters: Value,
    pub source: ToolSource,
    #[serde(default)]
    pub dangerous: bool,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// 参数 JSON Schema
    fn schema(&self) -> Value;
    /// 危险工具默认禁用,需要 `[tools].allow_dangerous = true` 才会执行
    fn dangerous(&self) -> bool {
        false
    }
    async fn execute(&self, args: Value) -> Result<Value>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    allowed: HashSet<String>,
    allow_dangerous: bool,
    timeout: Duration,
}

impl ToolRegistry {
    pub fn new(cfg: &AppConfig) -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
            allowed: cfg.tools.enabled.iter().cloned().collect(),
            allow_dangerous: cfg.tools.allow_dangerous,
            timeout: Duration::from_secs(cfg.tools.execute_timeout_secs.max(1)),
        };
        registry.register(Arc::new(builtin::GetTimeTool));
        registry.register(Arc::new(builtin::EchoTool));
        registry.register(Arc::new(builtin::HttpGetTool::new(cfg.tools.http_get.clone())));
        registry
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tools.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 白名单判定:未列入 `[tools].enabled` 的工具不暴露给模型,也不允许执行。
    pub fn is_allowed(&self, name: &str) -> bool {
        self.allowed.contains(name)
    }

    pub fn allow_dangerous(&self) -> bool {
        self.allow_dangerous
    }

    /// 暴露给模型的工具清单(只含已启用且允许的工具)。
    pub fn specs(&self) -> Vec<ToolSpec> {
        let mut out = Vec::new();
        for name in self.names() {
            if !self.is_allowed(&name) {
                continue;
            }
            let Some(tool) = self.tools.get(&name) else {
                continue;
            };
            if tool.dangerous() && !self.allow_dangerous {
                continue;
            }
            out.push(ToolSpec {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.schema(),
                source: ToolSource::Builtin,
                dangerous: tool.dangerous(),
            });
        }
        out
    }

    /// 按名字取工具描述,同时校验"是否存在 + 是否在白名单"。
    /// 模型永远只看到本次请求真正允许用的工具。
    pub fn specs_for(&self, names: &[String]) -> Result<Vec<ToolSpec>> {
        let mut out = Vec::new();
        for name in names {
            if !self.contains(name) {
                return Err(AppError::ToolNotAllowed(format!(
                    "{name}(未注册;可用: {:?})",
                    self.names()
                )));
            }
            if !self.is_allowed(name) {
                return Err(AppError::ToolNotAllowed(format!(
                    "{name}(被 [tools].enabled 白名单拒绝)"
                )));
            }
            if let Some(spec) = self.specs().into_iter().find(|s| &s.name == name) {
                out.push(spec);
            }
        }
        Ok(out)
    }

    pub async fn execute(&self, name: &str, args: Value) -> Result<Value> {
        if !self.is_allowed(name) {
            return Err(AppError::ToolNotAllowed(name.to_string()));
        }
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| AppError::ToolNotAllowed(name.to_string()))?
            .clone();
        if tool.dangerous() && !self.allow_dangerous {
            return Err(AppError::Tool(format!(
                "工具 `{name}` 属于危险工具且未启用([tools].allow_dangerous = false)"
            )));
        }
        let args = normalize_args(args);
        validate_args(&tool.schema(), &args).map_err(|reason| AppError::ToolInvalidArgs {
            tool: name.to_string(),
            reason,
        })?;

        match tokio::time::timeout(self.timeout, tool.execute(args)).await {
            Ok(res) => res,
            Err(_) => Err(AppError::Tool(format!(
                "工具 `{name}` 执行超过 {} 秒被终止",
                self.timeout.as_secs()
            ))),
        }
    }
}

/// 模型经常把 arguments 写成 JSON 字符串(`"{\"a\":1}"`),这里统一纠正为对象。
pub fn normalize_args(args: Value) -> Value {
    match args {
        Value::Null => json!({}),
        Value::String(s) => serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s)),
        other => other,
    }
}

/// 轻量 JSON Schema 子集校验:够覆盖工具参数,不引入完整 schema 引擎。
/// 支持 type / required / properties / enum / items / additionalProperties / minimum / maximum / maxLength。
pub fn validate_args(schema: &Value, args: &Value) -> std::result::Result<(), String> {
    validate_node("", schema, args)
}

fn type_matches(expected: &str, value: &Value) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "null" => value.is_null(),
        _ => true, // 未知类型不阻断
    }
}

fn validate_node(path: &str, schema: &Value, value: &Value) -> std::result::Result<(), String> {
    let Some(obj) = schema.as_object() else {
        return Ok(());
    };
    let here = if path.is_empty() { "参数".to_string() } else { path.to_string() };

    if let Some(expected) = obj.get("type").and_then(Value::as_str) {
        if !type_matches(expected, value) {
            return Err(format!("{here} 应为 {expected},实际是 {}", json_type(value)));
        }
    }

    if let Some(allowed) = obj.get("enum").and_then(Value::as_array) {
        if !allowed.iter().any(|a| a == value) {
            return Err(format!("{here} 取值不在允许集合 {allowed:?} 内"));
        }
    }

    if let Some(max_len) = obj.get("maxLength").and_then(Value::as_u64) {
        if let Some(s) = value.as_str() {
            if s.chars().count() as u64 > max_len {
                return Err(format!("{here} 长度超过 {max_len}"));
            }
        }
    }

    if let Some(min) = obj.get("minimum").and_then(Value::as_f64) {
        if let Some(n) = value.as_f64() {
            if n < min {
                return Err(format!("{here} 小于最小值 {min}"));
            }
        }
    }
    if let Some(max) = obj.get("maximum").and_then(Value::as_f64) {
        if let Some(n) = value.as_f64() {
            if n > max {
                return Err(format!("{here} 大于最大值 {max}"));
            }
        }
    }

    if let Some(props) = obj.get("properties").and_then(Value::as_object) {
        if let Some(required) = obj.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                let present = value
                    .get(key)
                    .map(|v| !v.is_null())
                    .unwrap_or(false);
                if !present {
                    return Err(format!("{here} 缺少必填字段 `{key}`"));
                }
            }
        }
        if let Some(map) = value.as_object() {
            if obj.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
                for key in map.keys() {
                    if !props.contains_key(key) {
                        return Err(format!("{here} 含未定义字段 `{key}`"));
                    }
                }
            }
            for (key, sub_schema) in props {
                if let Some(sub_value) = map.get(key) {
                    if !sub_value.is_null() {
                        validate_node(&format!("{here}.{key}"), sub_schema, sub_value)?;
                    }
                }
            }
        }
    }

    if let Some(items) = obj.get("items") {
        if let Some(list) = value.as_array() {
            for (i, item) in list.iter().enumerate() {
                validate_node(&format!("{here}[{i}]"), items, item)?;
            }
        }
    }
    Ok(())
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_encoded_args_are_normalized() {
        let args = normalize_args(Value::String(r#"{"city":"上海"}"#.into()));
        assert_eq!(args["city"], "上海");
        assert_eq!(normalize_args(Value::Null), json!({}));
    }

    #[test]
    fn validation_reports_missing_required() {
        let schema = json!({
            "type": "object",
            "properties": { "city": { "type": "string", "maxLength": 20 } },
            "required": ["city"],
            "additionalProperties": false
        });
        assert!(validate_args(&schema, &json!({"city":"北京"})).is_ok());
        assert!(validate_args(&schema, &json!({})).unwrap_err().contains("city"));
        assert!(validate_args(&schema, &json!({"city":1})).unwrap_err().contains("应为 string"));
        assert!(validate_args(&schema, &json!({"city":"北京","x":1}))
            .unwrap_err()
            .contains("未定义字段"));
        assert!(validate_args(&schema, &json!({"city":"一二三四五六七八九十一二三四五六七八九十一二三四"}))
            .unwrap_err()
            .contains("长度超过"));
    }

    #[test]
    fn validation_checks_arrays_and_enums() {
        let schema = json!({
            "type": "object",
            "properties": {
                "mode": { "enum": ["a", "b"] },
                "ids": { "type": "array", "items": { "type": "integer" } }
            }
        });
        assert!(validate_args(&schema, &json!({"mode":"a","ids":[1,2]})).is_ok());
        assert!(validate_args(&schema, &json!({"mode":"c"})).unwrap_err().contains("允许集合"));
        assert!(validate_args(&schema, &json!({"ids":["1"]})).unwrap_err().contains("integer"));
    }

    #[test]
    fn registry_enforces_whitelist_and_dangerous_gate() {
        use crate::config::AppConfig;
        let mut cfg = AppConfig::default();
        // 只启用 get_time,其余工具一律拒绝
        cfg.tools.enabled = vec!["get_time".into()];
        cfg.tools.allow_dangerous = false;
        let registry = ToolRegistry::new(&cfg);

        let specs = registry.specs();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "get_time");
        assert!(!registry.is_allowed("http_get"));
    }

    #[tokio::test]
    async fn unknown_tool_is_rejected() {
        let cfg = AppConfig::default();
        let registry = ToolRegistry::new(&cfg);
        let err = registry.execute("rm_rf", json!({})).await.unwrap_err();
        assert!(matches!(err, AppError::ToolNotAllowed(_)), "应命中白名单:{err}");
    }

    #[tokio::test]
    async fn invalid_args_are_rejected_before_execution() {
        let cfg = AppConfig::default();
        let registry = ToolRegistry::new(&cfg);
        let err = registry
            .execute("get_time", json!({"offset": 42}))
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::ToolInvalidArgs { .. }), "应是参数校验失败:{err}");
    }
}
