//! 内置示例工具:get_time / echo / http_get。
//!
//! `http_get` 是唯一有出网能力的工具,因此默认**拒绝所有主机**,
//! 必须在 `[tools.http_get].allowed_hosts` 里显式加域名,并且额外挡住内网/元数据地址。

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::HttpGetSettings;
use crate::error::{AppError, Result};
use crate::tools::registry::Tool;

pub struct GetTimeTool;

#[async_trait]
impl Tool for GetTimeTool {
    fn name(&self) -> &str {
        "get_time"
    }

    fn description(&self) -> &str {
        "获取当前日期时间。可指定 UTC 或 ±HH:MM 形式的固定偏移(如 +08:00),默认本机时区。"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "offset": {
                    "type": "string",
                    "maxLength": 16,
                    "description": "UTC 或 ±HH:MM 形式的固定偏移,例如 UTC、+08:00、-05:30。留空使用本机时区。"
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let raw = args
            .get("offset")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();

        // 只支持固定偏移:IANA 时区名需要额外依赖,而这个工具只需要回答"现在几点"。
        let (datetime, used) = if raw.is_empty() {
            (chrono::Local::now().to_rfc3339(), "local".to_string())
        } else {
            let normalized = match raw.to_ascii_uppercase().as_str() {
                "UTC" | "Z" => "+00:00".to_string(),
                _ => raw.clone(),
            };
            let offset: chrono::FixedOffset = normalized.parse().map_err(|e| {
                AppError::Tool(format!(
                    "无法解析偏移 `{raw}`: {e}。只支持 UTC 或 ±HH:MM,例如 +08:00"
                ))
            })?;
            (
                chrono::Utc::now().with_timezone(&offset).to_rfc3339(),
                normalized,
            )
        };

        Ok(json!({
            "datetime": datetime,
            "timezone": used,
            "unix": chrono::Utc::now().timestamp(),
        }))
    }
}

pub struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "原样回显参数,用于验证工具调用链路是否连通。"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "maxLength": 2000, "description": "要回显的文本" }
            },
            "required": ["text"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let text = args
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        Ok(json!({ "text": text, "length": text.chars().count() }))
    }
}

pub struct HttpGetTool {
    settings: HttpGetSettings,
    client: reqwest::Client,
}

impl HttpGetTool {
    pub fn new(settings: HttpGetSettings) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(settings.timeout_secs.max(1)))
            // 出网工具不跟随跳转,避免白名单被 30x 绕过
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("DS_WebAgent/0.1 (+local tool)")
            .build()
            .unwrap_or_default();
        Self { settings, client }
    }

    fn host_allowed(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        self.settings.allowed_hosts.iter().any(|pattern| {
            let pattern = pattern.trim().to_ascii_lowercase();
            if pattern.is_empty() {
                return false;
            }
            match pattern.strip_prefix("*.") {
                Some(suffix) => host == suffix || host.ends_with(&format!(".{suffix}")),
                None => host == pattern,
            }
        })
    }
}

/// 内网 / 链路本地 / 云元数据地址:即使被加进白名单也只可能是误配,直接拒绝。
fn is_private_or_metadata_host(host: &str) -> bool {
    let host = host.trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase();
    if host == "localhost"
        || host.ends_with(".localhost")
        || host == "metadata.google.internal"
        || host == "metadata"
    {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.octets()[0] == 0
        }
        Ok(std::net::IpAddr::V6(v6)) => {
            v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
        Err(_) => false,
    }
}

#[async_trait]
impl Tool for HttpGetTool {
    fn name(&self) -> &str {
        "http_get"
    }

    fn description(&self) -> &str {
        "对白名单内的主机发起一次 GET 请求并返回文本内容(响应会被截断)。"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "maxLength": 2048, "description": "完整 http/https URL" },
                "max_bytes": { "type": "integer", "minimum": 1, "maximum": 1048576 }
            },
            "required": ["url"],
            "additionalProperties": false
        })
    }

    fn dangerous(&self) -> bool {
        // 唯一的出网能力,按危险工具处理:默认禁用
        true
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let raw = args
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Tool("缺少 url 参数".into()))?;
        let url = reqwest::Url::parse(raw)
            .map_err(|e| AppError::Tool(format!("URL 不合法 `{raw}`: {e}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::Tool(format!(
                "仅支持 http/https,收到 {}",
                url.scheme()
            )));
        }
        let host = url
            .host_str()
            .ok_or_else(|| AppError::Tool("URL 缺少主机名".into()))?
            .to_string();

        if self.settings.allowed_hosts.is_empty() {
            return Err(AppError::Tool(
                "http_get 出网白名单为空,已拒绝一切请求。请在 config.toml 的 \
                 [tools.http_get].allowed_hosts 中显式添加允许的域名"
                    .into(),
            ));
        }
        if !self.host_allowed(&host) {
            return Err(AppError::Tool(format!(
                "主机 `{host}` 不在白名单内({:?})",
                self.settings.allowed_hosts
            )));
        }
        if is_private_or_metadata_host(&host) {
            return Err(AppError::Tool(format!(
                "拒绝访问内网/元数据地址 `{host}`"
            )));
        }

        let limit = args
            .get("max_bytes")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(self.settings.max_bytes)
            .min(self.settings.max_bytes);

        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| AppError::Tool(format!("请求 {url} 失败: {e}")))?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();

        let body = response
            .text()
            .await
            .map_err(|e| AppError::Tool(format!("读取响应体失败: {e}")))?;
        let truncated = body.len() > limit;
        let text: String = body.chars().take(limit).collect();

        Ok(json!({
            "status": status,
            "content_type": content_type,
            "truncated": truncated,
            "body": text,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_allowlist_supports_wildcard_suffix() {
        let tool = HttpGetTool::new(HttpGetSettings {
            allowed_hosts: vec!["example.com".into(), "*.wikipedia.org".into()],
            ..Default::default()
        });
        assert!(tool.host_allowed("example.com"));
        assert!(tool.host_allowed("zh.wikipedia.org"));
        assert!(tool.host_allowed("wikipedia.org"));
        assert!(!tool.host_allowed("evil.com"));
        assert!(!tool.host_allowed("example.com.evil.com"));
    }

    #[test]
    fn private_and_metadata_hosts_are_blocked() {
        for host in [
            "localhost",
            "127.0.0.1",
            "10.0.0.5",
            "192.168.1.1",
            "172.16.0.1",
            "169.254.169.254",
            "[::1]",
            "metadata.google.internal",
        ] {
            assert!(is_private_or_metadata_host(host), "{host} 应被拒绝");
        }
        assert!(!is_private_or_metadata_host("example.com"));
    }

    #[tokio::test]
    async fn http_get_refuses_when_allowlist_empty() {
        let tool = HttpGetTool::new(HttpGetSettings::default());
        let err = tool
            .execute(json!({"url": "https://example.com"}))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("白名单"), "应提示配置白名单: {err}");
    }

    #[tokio::test]
    async fn http_get_refuses_non_whitelisted_host() {
        let tool = HttpGetTool::new(HttpGetSettings {
            allowed_hosts: vec!["example.com".into()],
            ..Default::default()
        });
        let err = tool
            .execute(json!({"url": "https://other.com"}))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("不在白名单"), "应拒绝非白名单主机: {err}");
    }

    #[test]
    fn time_tool_accepts_utc_and_offsets() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for offset in ["UTC", "+08:00", "-05:30", ""] {
            let out = rt
                .block_on(GetTimeTool.execute(json!({"offset": offset})))
                .unwrap_or_else(|e| panic!("offset {offset:?} 应可用: {e}"));
            assert!(out.get("datetime").is_some());
        }
    }

    #[test]
    fn time_tool_rejects_bad_offset() {
        let err = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(GetTimeTool.execute(json!({"offset": "Mars/Olympus"})))
            .unwrap_err()
            .to_string();
        assert!(err.contains("无法解析偏移"), "{err}");
    }
}
