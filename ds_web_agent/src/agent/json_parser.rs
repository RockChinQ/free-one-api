//! 模型输出解析:从自由文本里提取 JSON 并校验成 [`ModelAction`]。
//!
//! 网页版模型不会严格只吐 JSON,现实中会遇到:前后有解释、包在 ```json 里、
//! 大括号前面有思考过程、`arguments` 被写成字符串。这里把这些情况全部处理掉,
//! 并且**永远返回可诊断的错误**,因为错误文本会作为修正提示回填给模型。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, Result};

/// 回填修正提示时,原文最多带回去多少字符(太长会把上下文迅速吃满)。
pub const RAW_ECHO_LIMIT: usize = 900;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelAction {
    ToolCall { tool: String, arguments: Value },
    Final { content: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractStrategy {
    /// 整段文本就是一个 JSON
    WholeText,
    /// 从 ```json 代码块里取
    FencedBlock,
    /// 用大括号配对扫描取(容忍前后有解释文本)
    BalancedBrace,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParsedAction {
    pub action: ModelAction,
    /// 原始 JSON(带 type/tool/arguments/content),直接回给 API 调用方
    pub model_json: Value,
    pub strategy: ExtractStrategy,
    pub raw: String,
}

/// 从文本中提取第一个可用的 JSON 对象。
pub fn extract_json(text: &str) -> Result<(Value, ExtractStrategy)> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AppError::JsonParse("模型输出为空".into()));
    }

    // 1) 整段就是 JSON
    if let Some(value) = parse_object(trimmed) {
        return Ok((value, ExtractStrategy::WholeText));
    }

    // 2) 代码块(优先认 type 字段,避免把示例 schema 当成答案)
    let fenced = fenced_blocks(trimmed);
    if let Some(value) = pick_best(fenced.into_iter().map(|s| (s, ExtractStrategy::FencedBlock))) {
        return Ok(value);
    }

    // 3) 大括号配对扫描
    let balanced = balanced_objects(trimmed)
        .into_iter()
        .map(|(s, e)| (trimmed[s..e].to_string(), ExtractStrategy::BalancedBrace));
    if let Some(value) = pick_best(balanced) {
        return Ok(value);
    }

    // 4) 引号修复后再扫一次:未转义的内层引号会把字符串状态带偏,原文里
    //    配对的括号常常扫不出来,修复后的文本才恢复正常的括号配对。
    let quoted = repair_json_quotes(trimmed);
    if quoted != trimmed {
        let balanced = balanced_objects(&quoted)
            .into_iter()
            .map(|(s, e)| (quoted[s..e].to_string(), ExtractStrategy::BalancedBrace));
        if let Some(value) = pick_best(balanced) {
            return Ok(value);
        }
    }

    Err(AppError::JsonParse(format!(
        "未能在输出中找到合法 JSON 对象(前 {} 字符:{})",
        RAW_ECHO_LIMIT.min(trimmed.len()),
        snippet(trimmed)
    )))
}

/// 解析一个候选 JSON 对象;直接解析失败时,先修复字符串里的裸控制字符再试,
/// 再不行就修未转义的内层双引号。
///
/// 网页模型经常把 `final.content` 写成带真实换行的字符串,这不符合 JSON 规范,
/// 但语义是明确的。把字符串内部的换行/制表符转义后即可正常解析;已经转义过的
/// `\n` 不会被二次转义。写 `execute_code` 时还会把代码里的 `"..."` 原样塞进
/// 字符串,由 [`repair_json_quotes`] 兜底。
fn parse_object(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if value.is_object() {
            return Some(value);
        }
    }
    let repaired = repair_json_strings(trimmed);
    if let Ok(value) = serde_json::from_str::<Value>(&repaired) {
        if value.is_object() {
            return Some(value);
        }
    }
    let quoted = repair_json_quotes(&repaired);
    if quoted != repaired {
        if let Ok(value) = serde_json::from_str::<Value>(&quoted) {
            if value.is_object() {
                return Some(value);
            }
        }
    }
    None
}

/// 转义 JSON 字符串内部的裸控制字符,保留字符串外部的空白与已有转义序列。
fn repair_json_strings(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 16);
    let mut in_string = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if !in_string {
            if ch == '"' {
                in_string = true;
            }
            out.push(ch);
            continue;
        }
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => {
                out.push(ch);
                escaped = true;
            }
            '"' => {
                out.push(ch);
                in_string = false;
            }
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// 转义 JSON 字符串内部未转义的双引号,保留真正的字符串结束引号。
///
/// 模型写 `execute_code` 时会把代码原文塞进 `"code":"..."`,代码里的
/// `"http.server 8899"` 不转义就会提前结束字符串,后面的 `}` 被当成字符串内容,
/// 整段再也扫不出配对的对象。判据:字符串里的 `"` 之后跳过空白若紧跟
/// `,` `}` `]` `:` 才是结束引号,否则是内容,补上反斜杠。
fn repair_json_quotes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 16);
    let mut in_string = false;
    let mut escaped = false;
    for (i, ch) in raw.char_indices() {
        if !in_string {
            if ch == '"' {
                in_string = true;
            }
            out.push(ch);
            continue;
        }
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => {
                out.push(ch);
                escaped = true;
            }
            '"' if closes_string(&raw[i + 1..]) => {
                out.push(ch);
                in_string = false;
            }
            '"' => out.push_str("\\\""),
            c => out.push(c),
        }
    }
    out
}

/// 引号之后(跳过空白)是否是 JSON 结构符号,即这个引号能否结束字符串。
fn closes_string(rest: &str) -> bool {
    match rest.trim_start().chars().next() {
        None => true,
        Some(',' | '}' | ']' | ':') => true,
        _ => false,
    }
}

/// 在所有候选中优先选"带 type 字段且能识别"的那个,其次选任意对象。
fn pick_best<I>(candidates: I) -> Option<(Value, ExtractStrategy)>
where
    I: Iterator<Item = (String, ExtractStrategy)>,
{
    let mut first_object: Option<(Value, ExtractStrategy)> = None;
    let mut last_action: Option<(Value, ExtractStrategy)> = None;
    for (raw, strategy) in candidates {
        let Some(value) = parse_object(&raw) else {
            continue;
        };
        if looks_like_action(&value) {
            // 出现多个 action-like 对象时,后面的通常是真答案——前面常是模型复述的协议示例
            last_action = Some((value, strategy));
            continue;
        }
        if first_object.is_none() {
            first_object = Some((value, strategy));
        }
    }
    last_action.or(first_object)
}

fn looks_like_action(value: &Value) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .map(|t| normalized_kind(t).is_some())
        .unwrap_or(false)
}

/// 取 ``` 围起来的代码块内容(含无语言标记的)。
fn fenced_blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let Some(end) = after.find("```") else {
            break;
        };
        let inner = &after[..end];
        // 去掉开头的语言标记(通常是 json 或 JSON)
        let body = inner
            .split_once('\n')
            .map(|(first, rest_body)| {
                if first.trim().eq_ignore_ascii_case("json")
                    || first.trim().is_empty()
                    || !first.contains(['{', '[', ':'])
                {
                    rest_body
                } else {
                    inner
                }
            })
            .unwrap_or(inner);
        out.push(body.to_string());
        rest = &after[end + 3..];
    }
    out
}

/// 扫描平衡的大括号对象,跳过字符串与转义字符内部的括号。
fn balanced_objects(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start: Option<usize> = None;
    let mut in_string = false;
    let mut escaped = false;

    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start.take() {
                            out.push((s, i + 1));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// 把 `type` 的常见变体归一化,模型偶尔会写 "tool" / "answer"。
fn normalized_kind(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
        "tool_call" | "toolcall" | "tool" | "function_call" | "call" => Some("tool_call"),
        "final" | "final_answer" | "answer" | "done" | "finish" => Some("final"),
        _ => None,
    }
}

fn snippet(text: &str) -> String {
    let taken: String = text.chars().take(RAW_ECHO_LIMIT).collect();
    taken.replace('\n', "\\n")
}

/// 解析成动作。任何不符合协议的地方都给出"可指导模型改正"的错误文本。
pub fn parse_action(text: &str) -> Result<ParsedAction> {
    let raw = text.to_string();
    let (value, strategy) = extract_json(text)?;
    let model_json = value.clone();

    let obj = value
        .as_object()
        .ok_or_else(|| AppError::ProtocolViolation("输出不是 JSON 对象".into()))?;

    let kind_raw = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::ProtocolViolation("缺少字符串字段 `type`".into()))?;
    let kind = normalized_kind(kind_raw).ok_or_else(|| {
        AppError::ProtocolViolation(format!(
            "`type` 只能是 tool_call 或 final,收到 `{kind_raw}`"
        ))
    })?;

    let action = match kind {
        "tool_call" => {
            let tool = obj
                .get("tool")
                .and_then(Value::as_str)
                .or_else(|| obj.get("name").and_then(Value::as_str))
                .unwrap_or_default()
                .trim()
                .to_string();
            if tool.is_empty() {
                return Err(AppError::ProtocolViolation(
                    "type=tool_call 时必须提供非空字符串字段 `tool`".into(),
                ));
            }
            let arguments = obj
                .get("arguments")
                .or_else(|| obj.get("args"))
                .or_else(|| obj.get("parameters"))
                .cloned()
                .unwrap_or_else(|| Value::Object(Default::default()));
            let arguments = crate::tools::registry::normalize_args(arguments);
            if !arguments.is_object() {
                return Err(AppError::ProtocolViolation(format!(
                    "tool_call 的 `arguments` 必须是对象,收到 {}",
                    arguments
                )));
            }
            ModelAction::ToolCall { tool, arguments }
        }
        "final" => {
            let content = obj
                .get("content")
                .or_else(|| obj.get("answer"))
                .or_else(|| obj.get("message"))
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            match content {
                Some(c) => ModelAction::Final { content: c },
                None => {
                    return Err(AppError::ProtocolViolation(
                        "type=final 时必须提供字符串字段 `content`".into(),
                    ))
                }
            }
        }
        _ => unreachable!("normalized_kind 只返回两种取值"),
    };

    Ok(ParsedAction {
        action,
        model_json,
        strategy,
        raw,
    })
}

/// JSON 解析失败时回填的修正提示。
pub fn correction_message(error: &str, raw: &str) -> String {
    let echo: String = raw.chars().take(RAW_ECHO_LIMIT).collect();
    format!(
        "你上一次的输出无法解析,已作废。\n\
         错误:{error}\n\
         你上次的原文(可能被截断):\n{echo}\n\n\
         请只重新输出一个合法 JSON 对象,不要 Markdown、不要解释、不要多余文本。\n\
         注意:字符串里的换行必须写成 \\n,不要输出裸换行;双引号必须写成 \\\"。\n\
         格式:{{\"type\":\"tool_call\",\"tool\":\"...\",\"arguments\":{{...}}}} \
         或 {{\"type\":\"final\",\"content\":\"...\"}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_bare_object() {
        let parsed = parse_action(r#"{"type":"final","content":"完成了"}"#).unwrap();
        assert_eq!(parsed.strategy, ExtractStrategy::WholeText);
        assert_eq!(
            parsed.action,
            ModelAction::Final {
                content: "完成了".into()
            }
        );
    }

    #[test]
    fn parses_fenced_block_with_preamble() {
        let text = "我先调用工具:\n```json\n{\"type\":\"tool_call\",\"tool\":\"get_time\",\"arguments\":{}}\n```\n请稍等";
        let parsed = parse_action(text).unwrap();
        assert_eq!(parsed.strategy, ExtractStrategy::FencedBlock);
        match parsed.action {
            ModelAction::ToolCall { tool, arguments } => {
                assert_eq!(tool, "get_time");
                assert_eq!(arguments, json!({}));
            }
            other => panic!("应为 tool_call,实际 {other:?}"),
        }
    }

    #[test]
    fn parses_balanced_object_with_surrounding_text() {
        let text = "思考完毕。{\"type\":\"final\",\"content\":\"答案是 42\"} 以上。";
        let parsed = parse_action(text).unwrap();
        assert_eq!(parsed.strategy, ExtractStrategy::BalancedBrace);
        assert_eq!(
            parsed.action,
            ModelAction::Final {
                content: "答案是 42".into()
            }
        );
    }

    #[test]
    fn ignores_schema_example_and_picks_real_answer() {
        // 模型先复述了协议示例,后面才是真答案:必须挑带 type 的那个
        let text = "格式示例:{\"type\":\"tool_call\",\"tool\":\"name\",\"arguments\":{}}\n\
                    但这次直接回答:{\"type\":\"final\",\"content\":\"直接给结论\"}";
        let parsed = parse_action(text).unwrap();
        assert_eq!(
            parsed.action,
            ModelAction::Final {
                content: "直接给结论".into()
            }
        );
    }

    #[test]
    fn nested_braces_inside_string_do_not_break_scan() {
        let text = "前缀 {\"type\":\"final\",\"content\":\"含 } 和 { 以及 \\\" 引号\"} 后缀";
        let parsed = parse_action(text).unwrap();
        match parsed.action {
            ModelAction::Final { content } => assert!(content.contains("含 } 和 {")),
            other => panic!("应为 final,实际 {other:?}"),
        }
    }

    #[test]
    fn raw_newlines_inside_a_string_are_repaired() {
        let text = "{\"type\":\"final\",\"content\":\"第一行\n第二行\"}";
        let parsed = parse_action(text).unwrap();
        match parsed.action {
            ModelAction::Final { content } => assert_eq!(content, "第一行\n第二行"),
            other => panic!("应为 final,实际 {other:?}"),
        }
    }

    #[test]
    fn raw_newlines_inside_a_balanced_object_are_repaired() {
        let text = "说明\n{\"type\":\"final\",\"content\":\"a\nb\"}\n结束";
        let parsed = parse_action(text).unwrap();
        assert_eq!(parsed.strategy, ExtractStrategy::BalancedBrace);
        match parsed.action {
            ModelAction::Final { content } => assert_eq!(content, "a\nb"),
            other => panic!("应为 final,实际 {other:?}"),
        }
    }

    #[test]
    fn repair_leaves_valid_escapes_untouched() {
        let text = r#"{"type":"final","content":"a\nb \"q\""}"#;
        assert_eq!(repair_json_strings(text), text);
    }

    #[test]
    fn unescaped_quotes_inside_a_string_are_repaired() {
        // 真实失败样本:模型把 shell 命令里的双引号原样写进了 JSON 字符串
        let text = "{\"type\":\"tool_call\",\"tool\":\"execute_code\",\"arguments\":{\"code\":\"subprocess.run('pkill -f \"http.server 8899\"', shell=True)\\ntime.sleep(1)\"}}";
        let parsed = parse_action(text).unwrap();
        match parsed.action {
            ModelAction::ToolCall { tool, arguments } => {
                assert_eq!(tool, "execute_code");
                let code = arguments["code"].as_str().unwrap();
                assert!(code.contains("\"http.server 8899\""), "{code}");
                assert!(code.contains("\ntime.sleep(1)"), "{code}");
            }
            other => panic!("应为 tool_call,实际 {other:?}"),
        }
    }

    #[test]
    fn unescaped_quotes_in_a_balanced_object_are_repaired() {
        let text = "说明\n{\"type\":\"final\",\"content\":\"他说 \"你好\" 就走了\"}\n结束";
        let parsed = parse_action(text).unwrap();
        match parsed.action {
            ModelAction::Final { content } => assert!(content.contains("\"你好\""), "{content}"),
            other => panic!("应为 final,实际 {other:?}"),
        }
    }

    #[test]
    fn quote_repair_leaves_valid_json_untouched() {
        let text = r#"{"type":"final","content":"a \"q\" b","arguments":{}}"#;
        assert_eq!(repair_json_quotes(text), text);
    }

    #[test]
    fn string_encoded_arguments_are_coerced() {
        let text = r#"{"type":"tool_call","tool":"echo","arguments":"{\"text\":\"hi\"}"}"#;
        let parsed = parse_action(text).unwrap();
        match parsed.action {
            ModelAction::ToolCall { arguments, .. } => assert_eq!(arguments["text"], "hi"),
            other => panic!("应为 tool_call,实际 {other:?}"),
        }
    }

    #[test]
    fn type_aliases_are_accepted() {
        assert!(parse_action(r#"{"type":"answer","content":"ok"}"#).is_ok());
        assert!(parse_action(r#"{"type":"tool","tool":"echo","arguments":{}}"#).is_ok());
    }

    #[test]
    fn missing_content_is_a_protocol_violation() {
        let err = parse_action(r#"{"type":"final"}"#).unwrap_err();
        assert!(matches!(err, AppError::ProtocolViolation(_)), "{err}");
        assert!(err.to_string().contains("content"));
    }

    #[test]
    fn unknown_type_is_rejected() {
        let err = parse_action(r#"{"type":"thought","content":"x"}"#).unwrap_err();
        assert!(err.to_string().contains("tool_call 或 final"));
    }

    #[test]
    fn empty_output_is_reported() {
        assert!(matches!(
            parse_action("   ").unwrap_err(),
            AppError::JsonParse(_)
        ));
    }

    #[test]
    fn correction_message_carries_error_and_original() {
        let msg = correction_message("缺少 content", r#"{"type":"final"}"#);
        assert!(msg.contains("缺少 content"));
        assert!(msg.contains(r#"{"type":"final"}"#));
        assert!(msg.contains("只重新输出一个合法 JSON 对象"));
    }

    #[test]
    fn long_output_is_echoed_but_truncated() {
        let long = "x".repeat(5000);
        let msg = correction_message("err", &long);
        // 提示里回填的原文要受控,不能把上下文一次吃满
        assert!(msg.chars().count() < 2000, "修正提示过长:{}", msg.chars().count());
    }
}
