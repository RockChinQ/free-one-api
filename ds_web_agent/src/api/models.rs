//! HTTP 层的请求/响应类型。
//!
//! 模型调用只有一条路径:`/v1/chat/completions` 的 OpenAI 兼容外观。
//! 本文件里的类型严格对齐官方规范(字段名、取值、流式分片形状),
//! 桥梁自己的附加信息只放进 `ds_webagent` 扩展字段。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::ClientToolSpec;
use crate::config::AppConfig;

/// 一次请求的用量视图:字符与 token 都给,金额按配置价目折算。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageView {
    pub rounds: usize,
    pub tool_calls: usize,
    /// 总用量:强提示词 + 载荷 + 模型返回 + 模型思考 + 工具结果回填
    pub consumed_chars: usize,
    pub consumed_tokens: usize,
    pub prompt_chars: usize,
    pub prompt_tokens: usize,
    pub reply_chars: usize,
    pub reply_tokens: usize,
    pub thinking_chars: usize,
    pub thinking_tokens: usize,
    pub tool_result_chars: usize,
    pub tool_result_tokens: usize,
    /// 有效用量:仅 API 入参与返回给客户端的内容
    pub effective_chars: usize,
    pub effective_tokens: usize,
    pub effective_input_tokens: usize,
    pub effective_output_tokens: usize,
    /// 本次节省的金额(按百万 token 价目折算有效用量)
    pub saved_amount: f64,
    pub currency: String,
}

impl UsageView {
    pub fn build(
        usage: &crate::agent::RunUsage,
        input_chars: usize,
        output_chars: usize,
        cfg: &AppConfig,
    ) -> Self {
        let t = |chars: usize| cfg.context.tokens_from_chars(chars);
        let input_tokens = t(input_chars);
        let output_tokens = t(output_chars);
        let saved = money(input_tokens, cfg.pricing.input_per_million)
            + money(output_tokens, cfg.pricing.output_per_million);
        Self {
            rounds: usage.rounds,
            tool_calls: usage.tool_calls,
            consumed_chars: usage.consumed_chars,
            consumed_tokens: t(usage.consumed_chars),
            prompt_chars: usage.prompt_chars,
            prompt_tokens: t(usage.prompt_chars),
            reply_chars: usage.reply_chars,
            reply_tokens: t(usage.reply_chars),
            thinking_chars: usage.thinking_chars,
            thinking_tokens: t(usage.thinking_chars),
            tool_result_chars: usage.tool_result_chars,
            tool_result_tokens: t(usage.tool_result_chars),
            effective_chars: usage.effective_chars,
            effective_tokens: input_tokens + output_tokens,
            effective_input_tokens: input_tokens,
            effective_output_tokens: output_tokens,
            saved_amount: round_money(saved),
            currency: cfg.pricing.currency.clone(),
        }
    }
}

pub fn money(tokens: usize, per_million: f64) -> f64 {
    (tokens as f64 / 1_000_000.0) * per_million
}

/// 金额保留到 1e-4,避免浮点噪声让同一个数字看起来每次都在变。
pub fn round_money(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

// ---------------------------------------------------------------- OpenAI 兼容

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiMessage {
    #[serde(default)]
    pub role: String,
    /// 可能是字符串,也可能是 parts 数组
    #[serde(default)]
    pub content: Value,
    /// assistant 消息里模型请求的工具调用(客户端回放历史时原样带回来)
    #[serde(default)]
    pub tool_calls: Option<Value>,
    /// role="tool" 时对应哪次调用
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// role="tool" 时的工具名(部分客户端会带)
    #[serde(default)]
    pub name: Option<String>,
}

impl OpenAiMessage {
    pub fn text(&self) -> String {
        match &self.content {
            Value::String(s) => s.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter_map(|p| {
                    p.get("text")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| p.as_str().map(str::to_string))
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Value::Null => String::new(),
            other => other.to_string(),
        }
    }

    /// 提取消息中的文件/图片路径列表。
    /// 支持 OpenAI 多模态格式:image_url 或 file_url 中的本地路径。
    pub fn extract_file_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        if let Value::Array(parts) = &self.content {
            for part in parts {
                // image_url 格式: {"type": "image_url", "image_url": {"url": "..."}}
                if part.get("type").and_then(Value::as_str) == Some("image_url") {
                    if let Some(url) = part
                        .pointer("/image_url/url")
                        .and_then(Value::as_str)
                    {
                        if let Some(path) = url_to_path(url) {
                            paths.push(path);
                        }
                    }
                }
                // file_url 格式: {"type": "file_url", "file_url": {"url": "..."}}
                if part.get("type").and_then(Value::as_str) == Some("file_url") {
                    if let Some(url) = part
                        .pointer("/file_url/url")
                        .and_then(Value::as_str)
                    {
                        if let Some(path) = url_to_path(url) {
                            paths.push(path);
                        }
                    }
                }
                // 直接是路径字符串
                if part.get("type").and_then(Value::as_str) == Some("file_path") {
                    if let Some(path) = part.get("path").and_then(Value::as_str) {
                        paths.push(path.to_string());
                    }
                }
            }
        }
        paths
    }
}

/// 从 URL 或路径中提取本地文件路径。
/// 支持 file:// 前缀和直接路径。
fn url_to_path(url: &str) -> Option<String> {
    if let Some(path) = url.strip_prefix("file://") {
        return Some(path.to_string());
    }
    // 检查是否是本地路径(不是 http/https/data 开头)
    if !url.starts_with("http://")
        && !url.starts_with("https://")
        && !url.starts_with("data:")
        && !url.starts_with("ftp://")
    {
        return Some(url.to_string());
    }
    None
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiChatRequest {
    pub model: Option<String>,
    #[serde(default)]
    pub messages: Vec<OpenAiMessage>,
    #[serde(default)]
    pub stream: bool,
    /// `stream_options.include_usage = true` 时,流式响应末尾额外补一个只带 usage 的分片
    pub stream_options: Option<StreamOptions>,
    /// OpenAI 风格的工具声明:工具由客户端定义、客户端执行,桥梁只负责转达
    pub tools: Option<Value>,
    /// `"none"` 本轮不给工具;`"auto"` / `"required"` 给全部声明;`{"type":"function",...}` 只给指定函数
    pub tool_choice: Option<Value>,
    /// 桥梁扩展:沿用同一条网页对话(省略则每次新建对话;标准客户端本来就不需要它)
    pub session_id: Option<String>,
    /// 桥梁扩展:单个请求最多几轮工具调用
    pub max_rounds: Option<usize>,
}

/// 标准里其它请求参数(`temperature` / `top_p` / `max_tokens` / `stop` / `n` / `seed` …)
/// 会被接受但不生效:网页对话不暴露这些旋钮,这里选择“接受并忽略”而不是报错。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StreamOptions {
    #[serde(default)]
    pub include_usage: bool,
}

/// 把 OpenAI 的 messages 拆成"历史转写 + 本轮输入"。
///
/// 网页对话是有状态的,客户端多轮里只有最后一轮是新的;更早的内容要作为前缀带进去。
pub fn split_openai_messages(messages: &[OpenAiMessage]) -> Option<(String, String)> {
    let last_user = messages.iter().rposition(|m| m.role == "user")?;
    let input = messages[last_user].text();
    let transcript = messages[..last_user]
        .iter()
        .filter(|m| !m.text().trim().is_empty())
        .map(|m| format!("{}: {}", m.role, m.text().trim()))
        .collect::<Vec<_>>()
        .join("\n");
    Some((transcript, input))
}

/// 解析 OpenAI `tools` 声明为客户端工具规格:名字/描述/参数 schema 全部来自请求,
/// 与本地注册表无关(所以客户端可以声明任意工具,例如 `get_weather`)。
pub fn parse_client_tools(tools: Option<&Value>) -> Vec<ClientToolSpec> {
    tools
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let f = item.get("function")?;
                    let name = f.get("name").and_then(Value::as_str)?.trim();
                    if name.is_empty() {
                        return None;
                    }
                    Some(ClientToolSpec {
                        name: name.to_string(),
                        description: f
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        parameters: f.get("parameters").cloned().unwrap_or_else(|| {
                            serde_json::json!({"type": "object", "properties": {}})
                        }),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 渲染 assistant 的 tool_calls,供后面某轮把历史带回给网页模型。
fn render_tool_calls(value: &Value) -> String {
    let Some(arr) = value.as_array() else {
        return String::new();
    };
    arr.iter()
        .filter_map(|c| {
            let name = c.pointer("/function/name").and_then(Value::as_str)?;
            let args = c
                .pointer("/function/arguments")
                .map(|a| match a {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_else(|| "{}".to_string());
            Some(format!("{name}({args})"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// 渲染客户端回传的工具结果。形状与桥梁内部的 TOOL_RESULT 一致,模型能直接读懂。
fn render_tool_result(m: &OpenAiMessage) -> String {
    let name = m.name.as_deref().unwrap_or("tool");
    let raw = m.text();
    let data = serde_json::from_str::<Value>(&raw).unwrap_or(Value::String(raw));
    let payload = serde_json::json!({ "tool": name, "ok": true, "data": data });
    format!("TOOL_RESULT\n{payload}")
}

/// 历史转写:assistant 的工具调用与 role=tool 的结果都要带上,
/// 否则多轮工具循环中模型看不到自己请求过什么、客户端返回了什么。
fn render_transcript(messages: &[OpenAiMessage]) -> String {
    messages
        .iter()
        .filter_map(|m| {
            let calls = m.tool_calls.as_ref().map(render_tool_calls).unwrap_or_default();
            let text = m.text();
            let trimmed = text.trim();
            let body = match (trimmed.is_empty(), calls.is_empty()) {
                (true, true) => return None,
                (false, true) => trimmed.to_string(),
                (true, false) => format!("调用工具 {calls}"),
                (false, false) => format!("{trimmed};调用工具 {calls}"),
            };
            Some(format!("{}: {body}", m.role))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// OpenAI 多轮往返:返回 (历史转写, 本轮输入)。
///
/// 本轮输入 = 末尾连续的 role="tool" 结果(客户端执行完工具回传),
/// 否则 = 最后一条 user 消息。客户端因此不需要理解桥梁的会话概念。
pub fn openai_turns(messages: &[OpenAiMessage]) -> Option<(String, String)> {
    if messages.is_empty() {
        return None;
    }
    let last = messages.len() - 1;
    if messages[last].role == "tool" {
        let mut start = last;
        while start > 0 && messages[start - 1].role == "tool" {
            start -= 1;
        }
        let input = messages[start..]
            .iter()
            .map(render_tool_result)
            .collect::<Vec<_>>()
            .join("\n");
        return Some((render_transcript(&messages[..start]), input));
    }
    let last_user = messages.iter().rposition(|m| m.role == "user")?;
    let input = messages[last_user].text();
    Some((render_transcript(&messages[..last_user]), input))
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAiChatResponse {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAiChoice>,
    pub usage: OpenAiUsage,
    /// 桥接自己的附加信息,标准客户端会忽略
    pub ds_webagent: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAiChoice {
    pub index: usize,
    pub message: OpenAiAssistantMessage,
    /// "stop" | "tool_calls"
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAiAssistantMessage {
    pub role: &'static str,
    /// 请求工具时为 null,与 OpenAI 一致
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
}

/// OpenAI 的工具调用:客户端据此执行工具,再用 `role: "tool"` 回传结果。
#[derive(Debug, Clone, Serialize)]
pub struct OpenAiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// 仅流式分片里出现,标识同一批调用中的序号
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    pub function: OpenAiToolFunction,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAiToolFunction {
    pub name: String,
    /// OpenAI 规定 arguments 是 JSON 字符串,不是对象
    pub arguments: String,
}

/// OpenAI `usage` 对象。`completion_tokens` 只算交付给客户端的内容,
/// 网页上的思考过程单独放进 `completion_tokens_details.reasoning_tokens`。
#[derive(Debug, Clone, Serialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub prompt_tokens_details: PromptTokensDetails,
    pub completion_tokens_details: CompletionTokensDetails,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PromptTokensDetails {
    pub cached_tokens: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CompletionTokensDetails {
    pub reasoning_tokens: usize,
}

impl OpenAiUsage {
    /// 只交付工具调用、没有正文时的用量。
    pub fn tool_call_only(prompt_tokens: usize, reasoning_tokens: usize) -> Self {
        Self {
            prompt_tokens,
            completion_tokens: 0,
            total_tokens: prompt_tokens,
            prompt_tokens_details: PromptTokensDetails::default(),
            completion_tokens_details: CompletionTokensDetails { reasoning_tokens },
        }
    }

    /// 由桥梁的用量视图折算:有效输入/输出 token + 网页上的思考 token。
    pub fn from_view(view: &UsageView) -> Self {
        Self {
            prompt_tokens: view.effective_input_tokens,
            completion_tokens: view.effective_output_tokens,
            total_tokens: view.effective_tokens,
            prompt_tokens_details: PromptTokensDetails::default(),
            completion_tokens_details: CompletionTokensDetails {
                reasoning_tokens: view.thinking_tokens,
            },
        }
    }
}

/// 流式增量。缺省字段一律不出现,与 OpenAI 的分片形状一致(纯结束分片的 delta 就是 `{}`)。
#[derive(Debug, Clone, Serialize)]
pub struct OpenAiDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAiChunkChoice {
    pub index: usize,
    pub delta: OpenAiDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAiChunk {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAiChunkChoice>,
    /// 只出现在 `stream_options.include_usage` 要求的那一个收尾分片上
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAiUsage>,
}

/// 一次流式响应内所有分片共用的头(id / model / created),规范要求它们逐片保持一致。
#[derive(Debug, Clone)]
pub struct OpenAiChunkMeta {
    pub id: String,
    pub model: String,
    pub created: i64,
}

impl OpenAiChunkMeta {
    pub fn new(model: &str) -> Self {
        Self {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            model: model.to_string(),
            created: chrono::Utc::now().timestamp(),
        }
    }
}

impl OpenAiChunk {
    fn new(meta: &OpenAiChunkMeta, delta: OpenAiDelta, finish_reason: Option<&str>) -> Self {
        Self {
            id: meta.id.clone(),
            object: "chat.completion.chunk",
            created: meta.created,
            model: meta.model.clone(),
            choices: vec![OpenAiChunkChoice {
                index: 0,
                delta,
                finish_reason: finish_reason.map(str::to_string),
            }],
            usage: None,
        }
    }

    /// 首个分片:声明角色。官方规范里 delta 带 `role`,后续分片只带内容。
    pub fn role(meta: &OpenAiChunkMeta) -> Self {
        Self::new(
            meta,
            OpenAiDelta {
                role: Some("assistant"),
                content: Some(String::new()),
                tool_calls: None,
            },
            None,
        )
    }

    pub fn content(meta: &OpenAiChunkMeta, content: &str) -> Self {
        Self::new(
            meta,
            OpenAiDelta {
                role: None,
                content: Some(content.to_string()),
                tool_calls: None,
            },
            None,
        )
    }

    /// 交付一次工具调用:整段内容一次性给出(网页回复本来就是整段)。
    pub fn tool_calls(meta: &OpenAiChunkMeta, call: &OpenAiToolCall) -> Self {
        let mut call = call.clone();
        call.index = Some(0);
        Self::new(
            meta,
            OpenAiDelta {
                role: None,
                content: None,
                tool_calls: Some(vec![call]),
            },
            None,
        )
    }

    pub fn finish(meta: &OpenAiChunkMeta, reason: &str) -> Self {
        Self::new(
            meta,
            OpenAiDelta {
                role: None,
                content: None,
                tool_calls: None,
            },
            Some(reason),
        )
    }

    /// `stream_options.include_usage` 的收尾分片:choices 为空数组,只带 usage。
    pub fn usage(meta: &OpenAiChunkMeta, usage: OpenAiUsage) -> Self {
        Self {
            id: meta.id.clone(),
            object: "chat.completion.chunk",
            created: meta.created,
            model: meta.model.clone(),
            choices: Vec::new(),
            usage: Some(usage),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_uses_per_million_pricing() {
        assert!((money(1_000_000, 9.0) - 9.0).abs() < 1e-9);
        assert!((money(500_000, 27.0) - 13.5).abs() < 1e-9);
        assert_eq!(money(0, 9.0), 0.0);
    }

    #[test]
    fn openai_messages_are_split_into_transcript_and_last_input() {
        let messages = vec![
            OpenAiMessage {
                role: "system".into(),
                content: Value::String("你是助手".into()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            OpenAiMessage {
                role: "user".into(),
                content: Value::String("第一问".into()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            OpenAiMessage {
                role: "assistant".into(),
                content: Value::String("第一答".into()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            OpenAiMessage {
                role: "user".into(),
                content: serde_json::json!([{"type": "text", "text": "第二问"}]),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];
        let (transcript, input) = split_openai_messages(&messages).expect("有 user 消息");
        assert_eq!(input, "第二问");
        assert!(transcript.contains("system: 你是助手"));
        assert!(transcript.contains("assistant: 第一答"));
        assert!(!transcript.contains("第二问"));
    }

    #[test]
    fn missing_user_message_yields_none() {
        let messages = vec![OpenAiMessage {
            role: "system".into(),
            content: Value::String("hi".into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];
        assert!(split_openai_messages(&messages).is_none());
    }

    #[test]
    fn client_tools_are_taken_from_request_verbatim() {
        // 客户端可以声明任意工具名,不需要本地注册(例如 get_weather)
        let tools = serde_json::json!([{
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "查天气",
                "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
            }
        }]);
        let specs = parse_client_tools(Some(&tools));
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "get_weather");
        assert_eq!(specs[0].description, "查天气");
        assert_eq!(specs[0].parameters["properties"]["city"]["type"], "string");
    }

    #[test]
    fn tool_result_turn_becomes_the_new_input() {
        let messages = vec![
            OpenAiMessage {
                role: "user".into(),
                content: Value::String("北京天气".into()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            OpenAiMessage {
                role: "assistant".into(),
                content: Value::Null,
                tool_calls: Some(serde_json::json!([{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "get_weather", "arguments": "{\"city\":\"北京\"}"}
                }])),
                tool_call_id: None,
                name: None,
            },
            OpenAiMessage {
                role: "tool".into(),
                content: Value::String("{\"temp_c\":21}".into()),
                tool_calls: None,
                tool_call_id: Some("call_1".into()),
                name: Some("get_weather".into()),
            },
        ];
        let (transcript, input) = openai_turns(&messages).expect("tool 结果能作为本轮输入");
        assert!(input.starts_with("TOOL_RESULT"));
        assert!(input.contains("get_weather"));
        assert!(input.contains("21"));
        // 历史里必须带上模型自己请求过的工具调用
        assert!(
            transcript.contains("get_weather({\"city\":\"北京\"})"),
            "{transcript}"
        );
        assert!(transcript.contains("北京"));
    }

    #[test]
    fn non_stream_response_has_the_required_fields() {
        let body = OpenAiChatResponse {
            id: "chatcmpl-test".into(),
            object: "chat.completion",
            created: 1_700_000_000,
            model: "ds-web-search".into(),
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiAssistantMessage {
                    role: "assistant",
                    content: Some("你好".into()),
                    tool_calls: None,
                },
                finish_reason: "stop".into(),
            }],
            usage: OpenAiUsage::tool_call_only(5, 0),
            ds_webagent: serde_json::json!({}),
        };
        let v = serde_json::to_value(&body).unwrap();
        for key in ["id", "object", "created", "model", "choices", "usage"] {
            assert!(v.get(key).is_some(), "响应缺字段 {key}: {v}");
        }
        assert_eq!(v["choices"][0]["index"], 0);
        assert_eq!(v["choices"][0]["message"]["role"], "assistant");
        assert_eq!(v["choices"][0]["finish_reason"], "stop");
        assert_eq!(v["usage"]["prompt_tokens"], 5);
        assert_eq!(v["usage"]["completion_tokens"], 0);
        assert_eq!(v["usage"]["total_tokens"], 5);
        // 没请求工具时不出 tool_calls 字段
        assert!(v["choices"][0]["message"].get("tool_calls").is_none());

        // 请求工具时 content 必须是 null(与 OpenAI 一致),arguments 是 JSON 字符串
        let with_call = OpenAiAssistantMessage {
            role: "assistant",
            content: None,
            tool_calls: Some(vec![OpenAiToolCall {
                id: "call_1".into(),
                kind: "function",
                index: None,
                function: OpenAiToolFunction {
                    name: "get_weather".into(),
                    arguments: "{\"city\":\"北京\"}".into(),
                },
            }]),
        };
        let v = serde_json::to_value(&with_call).unwrap();
        assert_eq!(v["content"], serde_json::Value::Null);
        assert!(v["tool_calls"][0]["function"]["arguments"].is_string());
        // 非流式里不带 index(只有流式分片才需要)
        assert!(v["tool_calls"][0].get("index").is_none());
    }

    #[test]
    fn stream_chunks_follow_the_openai_shape() {
        let meta = OpenAiChunkMeta::new("ds-web-search");
        let chunks: Vec<serde_json::Value> = [
            OpenAiChunk::role(&meta),
            OpenAiChunk::content(&meta, "你好"),
            OpenAiChunk::finish(&meta, "stop"),
        ]
        .iter()
        .map(|c| serde_json::to_value(c).unwrap())
        .collect();

        for chunk in &chunks {
            assert_eq!(chunk["id"], meta.id.as_str(), "分片 id 必须一致");
            assert_eq!(chunk["created"], meta.created, "分片 created 必须一致");
            assert_eq!(chunk["object"], "chat.completion.chunk");
            assert_eq!(chunk["model"], "ds-web-search");
        }
        // 首片声明角色,且此时还没有 finish_reason
        assert_eq!(chunks[0]["choices"][0]["delta"]["role"], "assistant");
        assert_eq!(chunks[0]["choices"][0]["delta"]["content"], "");
        assert_eq!(chunks[0]["choices"][0]["finish_reason"], serde_json::Value::Null);
        // 后续分片只带出现的字段;结束分片的 delta 就是空对象
        assert_eq!(chunks[1]["choices"][0]["delta"]["content"], "你好");
        assert!(chunks[1]["choices"][0]["delta"].get("role").is_none());
        assert_eq!(chunks[2]["choices"][0]["delta"], serde_json::json!({}));
        assert_eq!(chunks[2]["choices"][0]["finish_reason"], "stop");

        // usage 分片:choices 是空数组,只带 usage
        let usage_chunk = serde_json::to_value(OpenAiChunk::usage(
            &meta,
            OpenAiUsage::tool_call_only(10, 3),
        ))
        .unwrap();
        assert_eq!(usage_chunk["choices"], serde_json::json!([]));
        assert_eq!(usage_chunk["usage"]["prompt_tokens"], 10);
        assert_eq!(usage_chunk["usage"]["total_tokens"], 10);
        assert_eq!(
            usage_chunk["usage"]["completion_tokens_details"]["reasoning_tokens"],
            3
        );
        // 没请求 include_usage 的普通分片不带 usage 字段
        assert!(chunks[0].get("usage").is_none());
    }
}
