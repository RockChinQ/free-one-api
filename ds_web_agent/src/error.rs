//! 统一错误类型:所有对外错误都映射为 OpenAI 标准的错误体
//! `{"error":{"message","type","param","code"}}`,便于现成客户端统一处理。
//! 桥梁自己的细分原因放在 `code` 里,`retryable` / `diagnostics` 是额外字段(标准客户端会忽略)。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("配置错误: {0}")]
    Config(String),

    #[error("浏览器错误: {0}")]
    Browser(String),

    #[error("浏览器尚未启动完成")]
    BrowserNotReady,

    #[error("尚未登录 DeepSeek:请在已打开的浏览器窗口中手动登录后重试")]
    NotLoggedIn,

    /// 选择器漂移是页面改版最常见的问题:错误里必须带诊断信息,只改 config.toml 即可修复。
    #[error("选择器未命中: {selector}(URL={url}, 标题={title}, 截图={screenshot})")]
    SelectorNotFound {
        selector: String,
        url: String,
        title: String,
        screenshot: String,
    },

    #[error("发送消息失败: {0}")]
    SendFailed(String),

    #[error("等待回复超时({0} 秒内未检测到稳定的新回复)")]
    ReplyTimeout(u64),

    #[error("模型未返回任何可读文本(conversation_id={0})")]
    EmptyReply(String),

    #[error("模型输出解析失败: {0}")]
    JsonParse(String),

    #[error("模型输出不符合协议: {0}")]
    ProtocolViolation(String),

    #[error("工具错误: {0}")]
    Tool(String),

    #[error("工具 `{0}` 未注册或未在白名单中")]
    ToolNotAllowed(String),

    #[error("工具 `{tool}` 参数校验失败: {reason}")]
    ToolInvalidArgs { tool: String, reason: String },

    #[error("超过最大轮次 {0}")]
    MaxRoundsExceeded(usize),

    #[error("请求被取消或客户端断开")]
    Cancelled,

    #[error("鉴权失败:Authorization 头缺失或 Bearer token 不匹配")]
    Unauthorized,

    #[error("请求参数错误: {0}")]
    BadRequest(String),

    #[error("会话不存在或已过期: {0}")]
    SessionNotFound(String),

    #[error("模型 `{0}` 不存在;可用模型见 GET /v1/models")]
    ModelNotFound(String),

    #[error("路径不存在: {0}")]
    NotFound(String),

    /// 对应 OpenAI 的 `invalid_request_error`:`param` 指出出问题的是哪个请求字段。
    /// message 直接给出原因,不再加"请求参数错误"前缀——错误类型已经写在 `type` 里。
    #[error("{message}")]
    InvalidRequest {
        message: String,
        param: Option<String>,
    },

    #[error("等待该会话上一个请求完成超时")]
    Busy,

    #[error("存储错误: {0}")]
    Storage(String),

    #[error("内部错误: {0}")]
    Internal(String),
}

impl AppError {
    /// 便于在任意 `Result<T, E: Display>` 上直接问号传播到 AppError。
    pub fn browser<E: std::fmt::Display>(err: E) -> Self {
        AppError::Browser(err.to_string())
    }

    pub fn internal<E: std::fmt::Display>(err: E) -> Self {
        AppError::Internal(err.to_string())
    }

    pub fn code(&self) -> &'static str {
        match self {
            AppError::Config(_) => "config_error",
            AppError::Browser(_) | AppError::BrowserNotReady => "browser_error",
            AppError::NotLoggedIn => "not_logged_in",
            AppError::SelectorNotFound { .. } => "selector_not_found",
            AppError::SendFailed(_) => "send_failed",
            AppError::ReplyTimeout(_) => "reply_timeout",
            AppError::EmptyReply(_) => "empty_reply",
            AppError::JsonParse(_) | AppError::ProtocolViolation(_) => "model_protocol_error",
            AppError::Tool(_) => "tool_error",
            AppError::ToolNotAllowed(_) => "tool_not_allowed",
            AppError::ToolInvalidArgs { .. } => "tool_invalid_args",
            AppError::MaxRoundsExceeded(_) => "max_rounds_exceeded",
            AppError::Cancelled => "cancelled",
            AppError::Unauthorized => "unauthorized",
            AppError::BadRequest(_) => "bad_request",
            AppError::InvalidRequest { .. } => "invalid_request_error",
            AppError::ModelNotFound(_) => "model_not_found",
            AppError::NotFound(_) => "not_found",
            AppError::SessionNotFound(_) => "session_not_found",
            AppError::Busy => "busy",
            AppError::Storage(_) => "storage_error",
            AppError::Internal(_) => "internal_error",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::BadRequest(_)
            | AppError::InvalidRequest { .. }
            | AppError::Cancelled
            | AppError::MaxRoundsExceeded(_) => StatusCode::BAD_REQUEST,
            AppError::ModelNotFound(_) | AppError::SessionNotFound(_) | AppError::NotFound(_) => {
                StatusCode::NOT_FOUND
            }
            AppError::SelectorNotFound { .. } | AppError::Browser(_) | AppError::BrowserNotReady => {
                StatusCode::BAD_GATEWAY
            }
            AppError::NotLoggedIn => StatusCode::SERVICE_UNAVAILABLE,
            AppError::ReplyTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            AppError::Busy => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// OpenAI 客户端按 `type` 分类错误,所以这里只用官方取值;桥梁的细分放在 `code` 里。
    pub fn error_type(&self) -> &'static str {
        match self {
            AppError::Unauthorized => "authentication_error",
            AppError::BadRequest(_)
            | AppError::InvalidRequest { .. }
            | AppError::ModelNotFound(_)
            | AppError::SessionNotFound(_)
            | AppError::NotFound(_) => "invalid_request_error",
            AppError::Busy => "rate_limit_error",
            AppError::NotLoggedIn => "overloaded_error",
            _ => "api_error",
        }
    }

    /// 错误体里的 `param`:指出哪个请求字段有问题,确定不了时为 null(与 OpenAI 一致)。
    pub fn param(&self) -> Option<&str> {
        match self {
            AppError::InvalidRequest { param, .. } => param.as_deref(),
            AppError::ModelNotFound(_) => Some("model"),
            _ => None,
        }
    }

    /// 该错误是否值得让客户端稍后原样重试。
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            AppError::ReplyTimeout(_) | AppError::Busy | AppError::Browser(_)
        )
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        // 选择器类错误额外带上诊断字段,客户端/用户不用翻日志就能定位
        let mut body = json!({
            "error": {
                "message": self.to_string(),
                "type": self.error_type(),
                "param": self.param(),
                "code": self.code(),
                "retryable": self.retryable(),
            }
        });
        if let AppError::SelectorNotFound {
            ref selector,
            ref url,
            ref title,
            ref screenshot,
        } = self
        {
            body["error"]["diagnostics"] = json!({
                "selector": selector,
                "url": url,
                "title": title,
                "screenshot": screenshot,
                "hint": "页面可能改版:请更新 config.toml 的 [selectors] 后重试",
            });
        }
        (status, axum::Json(body)).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(format!("{err:#}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::JsonParse(err.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Storage(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Storage(err.to_string())
    }
}
