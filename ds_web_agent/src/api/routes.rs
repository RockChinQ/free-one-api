//! 本地 HTTP API(axum)。
//!
//! 模型调用只有一条路径:**OpenAI 兼容的 `/v1/chat/completions`**;其余端点都是管理/观测用途。
//! 路由分两组:
//! - 公开:`/health`、`/v1/health`(进程活着但还没登录时,客户端需要能问"现在能用了没"),
//!   以及 `/v1/stats`(只读用量统计,故意不鉴权,方便直接在浏览器里打开看);
//! - 受保护:其余全部走 Bearer 鉴权(`server.api_token` 为空时表示仅本机使用、不校验)。

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use serde_json::{json, Value};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::agent::{AgentLoop, AgentRun, ClientToolSpec};
use crate::api::models::*;
use crate::browser::{BrowserAdapter, LoginState};
use crate::config::AppConfig;
use crate::conversation::{ChatBackend, ClientTurn};
use crate::error::{AppError, Result};
use crate::storage::Store;
use crate::tools::ToolRegistry;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<AppConfig>,
    pub browser: Arc<dyn BrowserAdapter>,
    pub backend: Arc<dyn ChatBackend>,
    pub agent: Arc<AgentLoop>,
    pub registry: Arc<ToolRegistry>,
    pub store: Arc<Store>,
}

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/v1/chat/completions", post(openai_chat_completions))
        .route("/v1/models", get(list_models))
        .route("/v1/models/{id}", get(retrieve_model))
        .route("/v1/sessions", get(list_sessions))
        .route("/v1/sessions/{id}", get(session_detail).delete(delete_session))
        .route("/v1/sessions/{id}/close", post(close_session))
        .route("/v1/tools", get(list_tools))
        .route("/v1/errors", get(errors))
        .route("/v1/diag", get(diag))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_token,
        ))
        .layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024));

    // 用量统计是只读的,故意不鉴权:直接拿浏览器打开就能看。服务只绑本机,不会因此对外暴露
    let public_stats = Router::new().route("/v1/stats", get(stats));

    Router::new()
        .route("/health", get(health))
        .route("/v1/health", get(health))
        .merge(public_stats)
        .merge(protected)
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .layer(ConcurrencyLimitLayer::new(
            state.cfg.server.max_concurrent_requests.max(1),
        ))
        .with_state(state)
}

/// 未匹配的路径也返回 OpenAI 风格错误体,而不是空白 404。
async fn not_found(req: Request) -> Response {
    AppError::NotFound(format!("{} {}", req.method(), req.uri().path())).into_response()
}

/// Bearer 鉴权。token 为空视为"只在本机用、不校验",但绝不把 token 写进日志。
async fn require_token(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let expected = state.cfg.server.api_token.trim();
    if !expected.is_empty() {
        let provided = req
            .headers()
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
            .unwrap_or_default();
        if provided != expected {
            return AppError::Unauthorized.into_response();
        }
    }
    next.run(req).await
}

// ------------------------------------------------------------------ 健康检查

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let (status, detail) = match state.browser.login_state().await {
        Ok(LoginState::LoggedIn) => ("ready", "已登录,可以调用 /v1/chat/completions".to_string()),
        Ok(LoginState::WaitingLogin) => (
            "login_required",
            "尚未登录:请在已打开的浏览器窗口里手动登录 DeepSeek".to_string(),
        ),
        Ok(LoginState::Starting) => (
            "starting",
            format!(
                "浏览器/页面尚未就绪(可能仍在加载,或 [selectors] 已漂移):{}",
                state.browser.describe_page().await
            ),
        ),
        Ok(LoginState::Failed) => (
            "browser_failed",
            format!("浏览器启动或页面加载失败:{}", state.browser.describe_page().await),
        ),
        Err(e) => ("starting", format!("浏览器尚未就绪:{e}")),
    };

    let body = json!({
        "status": status,
        "ready": status == "ready",
        "detail": detail,
        "home_url": state.cfg.browser.home_url,
        "port": state.cfg.server.port,
        "models": state.cfg.models.variants.iter().map(|v| json!({
            "id": v.id,
            "label": v.label,
            "search": v.search,
            "think": v.think,
            "mode": v.mode_name(),
        })).collect::<Vec<_>>(),
        "tools": state.registry.specs().iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
    });
    // 还没登录时不返回 200/ready,但也不能报 5xx:这是"等待用户操作"的正常状态
    let code = if status == "ready" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(body))
}

/// 严格解析模型:不认识的 `model` 一律 404 `model_not_found`(与 OpenAI 一致),绝不静默换模型。
fn resolve_model(state: &AppState, requested: Option<&str>) -> Result<crate::config::ModelVariant> {
    let cfg = &state.cfg;
    let Some(id) = requested.map(str::trim).filter(|id| !id.is_empty()) else {
        return cfg
            .models
            .default_variant()
            .cloned()
            .ok_or_else(|| AppError::Config("models.default 未配置或不在 variants 中".into()));
    };
    cfg.models
        .find(id)
        .cloned()
        .ok_or_else(|| AppError::ModelNotFound(id.to_string()))
}

// ------------------------------------------------------------------ OpenAI 兼容(唯一的模型调用入口)

/// axum 自己的 JSON 提取失败也要变成 OpenAI 风格错误体(默认会是 text/plain)。
fn invalid_body(rejection: JsonRejection) -> AppError {
    AppError::InvalidRequest {
        message: format!("请求体不合法:{}", rejection.body_text()),
        param: None,
    }
}

/// 按 OpenAI 的 `tool_choice` 语义挑选本轮交给模型的工具:
/// - `"none"`:本轮不提供任何工具;
/// - `"auto"` / `"required"` / 省略:提供请求里声明的全部工具;
/// - `{"type":"function","function":{"name":"x"}}`:只提供 x,且 x 必须已在 `tools` 里声明。
fn select_client_tools(req: &OpenAiChatRequest) -> Result<Vec<ClientToolSpec>> {
    let declared = parse_client_tools(req.tools.as_ref());
    match req.tool_choice.as_ref() {
        None | Some(Value::Null) => Ok(declared),
        Some(Value::String(s)) if s == "none" => Ok(Vec::new()),
        Some(Value::String(s)) if s == "auto" || s == "required" => Ok(declared),
        Some(Value::String(other)) => Err(AppError::InvalidRequest {
            message: format!(
                "tool_choice 只支持 \"none\" / \"auto\" / \"required\" 或 {{\"type\":\"function\"}},收到 `{other}`"
            ),
            param: Some("tool_choice".into()),
        }),
        Some(choice) => {
            let name = choice
                .pointer("/function/name")
                .and_then(Value::as_str)
                .ok_or_else(|| AppError::InvalidRequest {
                    message: "tool_choice 指定函数时必须带 function.name".into(),
                    param: Some("tool_choice".into()),
                })?;
            if !declared.iter().any(|t| t.name == name) {
                return Err(AppError::InvalidRequest {
                    message: format!("tool_choice 指定的 `{name}` 不在 tools 里"),
                    param: Some("tool_choice".into()),
                });
            }
            Ok(declared.into_iter().filter(|t| t.name == name).collect())
        }
    }
}

async fn openai_chat_completions(
    State(state): State<AppState>,
    payload: std::result::Result<Json<OpenAiChatRequest>, JsonRejection>,
) -> Result<Response> {
    let Json(req) = payload.map_err(invalid_body)?;

    let (transcript, last_input) = openai_turns(&req.messages).ok_or_else(|| {
        AppError::InvalidRequest {
            message: "messages 里没有可处理的 user/tool 内容".into(),
            param: Some("messages".into()),
        }
    })?;

    // 工具由客户端声明、客户端执行:本地不注册也不执行,只把声明注入强提示词,
    // 模型请求时原样以 tool_calls 交还,等客户端用 role="tool" 回传结果后再来一轮。
    let client_tools = select_client_tools(&req)?;

    let model = resolve_model(&state, req.model.as_deref())?;
    let model_id = model.id.clone();

    // 提取消息中的文件/图片路径
    let mut file_paths = Vec::new();
    for msg in &req.messages {
        file_paths.extend(msg.extract_file_paths());
    }

    // 客户端回放的历史、锚点、本轮输入行都原样交给 AgentLoop 与记账逻辑:
    // 下一轮靠它们识别"是不是同一条对话",命中就沿用同一个网页对话(不新建、不重复注入)并只发增量。
    let anchor = conversation_anchor(&req.messages);
    let input_line = input_history_line(&req.messages);
    let run = AgentRun {
        session_id: req.session_id.clone(),
        input: last_input,
        transcript: transcript.clone(),
        anchor: anchor.clone(),
        tools: Vec::new(),
        client_tools,
        max_rounds: state.cfg.clamp_max_rounds(req.max_rounds),
        model,
        events: None,
        cancel: None,
        file_paths,
    };
    let input_chars = run.client_input().chars().count();
    let meta = OpenAiChunkMeta::new(&model_id);

    let outcome = state.agent.run(run).await.inspect_err(|e| {
        state
            .store
            .record_error(None, e.code(), &e.to_string(), &format!("model={model_id}"));
    })?;

    // 本轮对客户端可见的往返记账:下一轮它回放这段答复,就能认出还是同一个对话。
    // 记账失败不让本轮答复作废,只是下一轮会退回整段回放。
    let turn = ClientTurn {
        anchor,
        transcript,
        input_line,
        answer_line: match &outcome.tool_call {
            Some(call) => {
                assistant_replay_line("", Some((&call.name, &call.arguments.to_string())))
            }
            None => assistant_replay_line(&outcome.content, None),
        },
    };
    if let Err(e) = state
        .backend
        .remember_client_turn(&outcome.session_id, turn)
        .await
    {
        tracing::warn!(session_id = %outcome.session_id, error = %e, "记录客户端往返失败:下一轮会退回整段回放");
    }

    if state.cfg.storage.record_http_bodies {
        state.store.append_event(
            "chat_request",
            &json!({
                "session_id": outcome.session_id,
                "conversation_id": outcome.conversation_id,
                "model": model_id,
                "input_chars": input_chars,
                "rounds": outcome.rounds,
                "final_chars": outcome.content.chars().count(),
                "stream": req.stream,
            }),
        );
    }

    let usage_view = UsageView::build(
        &outcome.usage,
        input_chars,
        outcome.content.chars().count(),
        &state.cfg,
    );
    let include_usage = req
        .stream_options
        .as_ref()
        .map(|o| o.include_usage)
        .unwrap_or(false);

    // 模型请求工具 → 交还客户端执行(本地不执行),finish_reason=tool_calls
    if let Some(call) = outcome.tool_call {
        let tool_call = OpenAiToolCall {
            id: call.id,
            kind: "function",
            index: None,
            function: OpenAiToolFunction {
                name: call.name.clone(),
                arguments: call.arguments.to_string(),
            },
        };
        let usage = OpenAiUsage::tool_call_only(
            state.cfg.context.tokens_from_chars(input_chars),
            usage_view.thinking_tokens,
        );
        let extension = json!({
            "session_id": outcome.session_id,
            "conversation_id": outcome.conversation_id,
            "rounds": outcome.rounds,
            "client_tool_call": call.name,
        });
        if req.stream {
            return Ok(sse_response(
                &meta,
                vec![
                    OpenAiChunk::role(&meta),
                    OpenAiChunk::tool_calls(&meta, &tool_call),
                    OpenAiChunk::finish(&meta, "tool_calls"),
                ],
                include_usage.then(|| usage.clone()),
            ));
        }
        let body = OpenAiChatResponse {
            id: meta.id,
            object: "chat.completion",
            created: meta.created,
            model: meta.model,
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiAssistantMessage {
                    role: "assistant",
                    content: None,
                    tool_calls: Some(vec![tool_call]),
                },
                finish_reason: "tool_calls".to_string(),
            }],
            usage,
            ds_webagent: extension,
        };
        return Ok(Json(body).into_response());
    }

    let content = outcome.content;
    let usage = OpenAiUsage::from_view(&usage_view);
    let extension = json!({
        "session_id": outcome.session_id,
        "conversation_id": outcome.conversation_id,
        "rounds": outcome.rounds,
        "rotations": outcome.rotations,
        "steps": outcome.steps,
        "usage": usage_view,
    });

    if !req.stream {
        let body = OpenAiChatResponse {
            id: meta.id,
            object: "chat.completion",
            created: meta.created,
            model: meta.model,
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiAssistantMessage {
                    role: "assistant",
                    content: Some(content),
                    tool_calls: None,
                },
                finish_reason: "stop".to_string(),
            }],
            usage,
            ds_webagent: extension,
        };
        return Ok(Json(body).into_response());
    }

    // 流式:网页对话是整段回复,所以先跑完整轮、再把结果按规范的分片形状吐出
    // (role → content → finish → [DONE]);逐字伪造进度只会掩盖真实等待。
    Ok(sse_response(
        &meta,
        vec![
            OpenAiChunk::role(&meta),
            OpenAiChunk::content(&meta, &content),
            OpenAiChunk::finish(&meta, "stop"),
        ],
        include_usage.then_some(usage),
    ))
}

/// 把一批分片编成 SSE 响应;请求了 `stream_options.include_usage` 时末尾补一个只带 usage 的分片,
/// 最后必定是 `data: [DONE]`。
fn sse_response(
    meta: &OpenAiChunkMeta,
    chunks: Vec<OpenAiChunk>,
    usage: Option<OpenAiUsage>,
) -> Response {
    let usage_chunk = usage.map(|usage| OpenAiChunk::usage(meta, usage));
    let stream = futures_util::stream::iter(chunks.into_iter().chain(usage_chunk).map(|chunk| {
        let data = serde_json::to_string(&chunk).unwrap_or_else(|_| "{}".to_string());
        std::result::Result::<Event, Infallible>::Ok(Event::default().data(data))
    }))
    .chain(futures_util::stream::once(async {
        std::result::Result::<Event, Infallible>::Ok(Event::default().data("[DONE]"))
    }));
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

// ------------------------------------------------------------------ 其他端点

async fn list_models(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "object": "list",
        "data": state.cfg.models.variants.iter().map(model_json).collect::<Vec<_>>(),
    }))
}

async fn retrieve_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    state
        .cfg
        .models
        .find(&id)
        .map(|v| Json(model_json(v)))
        .ok_or_else(|| AppError::ModelNotFound(id))
}

/// 单个模型对象:标准字段 `id` / `object` / `created` / `owned_by`,其余是桥梁附加信息。
fn model_json(v: &crate::config::ModelVariant) -> Value {
    json!({
        "id": v.id,
        "object": "model",
        "created": model_created_at(),
        "owned_by": "ds_webagent",
        "label": v.label,
        "capabilities": { "search": v.search, "think": v.think },
    })
}

/// `created` 只是信息性字段:进程内保持不变,免得同一个模型每次请求都在变。
fn model_created_at() -> i64 {
    static CREATED_AT: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *CREATED_AT.get_or_init(|| chrono::Utc::now().timestamp())
}

async fn list_sessions(State(state): State<AppState>) -> Result<Json<Value>> {
    let live = state.backend.list_sessions().await?;
    Ok(Json(json!({
        "live": live,
        "recent": state.store.recent_sessions(50)?,
    })))
}

async fn session_detail(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>> {
    let record = state.backend.state(&id).await?;
    let stats = state.store.recent_sessions(200)?;
    let persisted = stats
        .into_iter()
        .find(|s| s.get("session_id").and_then(Value::as_str) == Some(id.as_str()));
    Ok(Json(json!({ "session": record, "persisted": persisted })))
}

async fn close_session(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>> {
    state.backend.close_session(&id).await?;
    Ok(Json(json!({ "closed": id })))
}

async fn delete_session(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>> {
    // 与 close 等价:只释放内存会话,不动浏览器里已经发生的对话
    state.backend.close_session(&id).await?;
    Ok(Json(json!({ "closed": id })))
}

async fn list_tools(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "allow_dangerous": state.registry.allow_dangerous(),
        "tools": state.registry.specs(),
    }))
}

async fn stats(State(state): State<AppState>) -> Result<Json<Value>> {
    let usage = state.store.usage()?;
    Ok(Json(json!({
        "usage": usage,
        "by_tool": state.store.tool_call_stats()?,
    })))
}

async fn errors(State(state): State<AppState>) -> Result<Json<Value>> {
    Ok(Json(json!({ "errors": state.store.recent_errors(50)? })))
}

/// 诊断端点:把当前页面的真实 URL/标题/输入框/头像元素 HTML 打出来,
/// 用于在选择器漂移时只改 config.toml 就能修复登录检测。
async fn diag(State(state): State<AppState>) -> Json<Value> {
    let page = state.browser.describe_page().await;
    let login = state.browser.login_state().await;
    let probe = r#"
        (function(){
            const q = s => document.querySelector(s);
            const input = q('textarea') || q('div[contenteditable="true"]');
            const av = q('div[class*="avatar"]') || q('img[class*="avatar"]')
                     || q('[data-testid*="user"]') || q('img[alt*="头像"]')
                     || q('[class*="user-info"]') || q('[aria-label*="账号"]');
            return JSON.stringify({
                url: location.href,
                title: document.title,
                input_tag: input ? input.tagName : null,
                input_outer: input ? input.outerHTML.slice(0, 400) : null,
                avatar_tag: av ? av.tagName : null,
                avatar_outer: av ? av.outerHTML.slice(0, 400) : null,
                has_stop: !!q('[class*="stop"]'),
            });
        })()
    "#;
    let probe_result = state.browser.eval_json(probe).await.ok();
    Json(json!({
        "describe_page": page,
        "login_state": format!("{:?}", login),
        "probe": probe_result,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MockChatBackend, NoopBrowser};

    fn state_with(cfg: AppConfig) -> AppState {
        state_with_backend(cfg, Arc::new(MockChatBackend::new(vec![])))
    }

    fn state_with_backend(cfg: AppConfig, backend: Arc<MockChatBackend>) -> AppState {
        let cfg = Arc::new(cfg);
        let store = Arc::new(Store::memory().expect("内存库"));
        let browser: Arc<dyn BrowserAdapter> = Arc::new(NoopBrowser::default());
        let backend_dyn: Arc<dyn ChatBackend> = backend;
        let registry = Arc::new(ToolRegistry::new(&cfg));
        let agent = Arc::new(AgentLoop::new(
            backend_dyn.clone(),
            registry.clone(),
            cfg.clone(),
            store.clone(),
        ));
        AppState {
            cfg,
            browser,
            backend: backend_dyn,
            agent,
            registry,
            store,
        }
    }

    /// 直接打一枪 `/v1/chat/completions`,拿到 JSON 响应体。
    async fn chat(app: &axum::Router, body: Value) -> Value {
        use axum::body::Body;
        use axum::http::Request as HttpRequest;
        use tower::ServiceExt;

        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .expect("路由可达");
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("读响应体");
        serde_json::from_slice(&bytes).expect("JSON 响应")
    }

    /// 一个网页对话要能聊很多轮:客户端第二轮不带 `session_id`,只要历史里回放着我们上一轮的
    /// 答复,就沿用同一个网页对话——不新建、不重复注入强提示词,而且只发增量。
    #[tokio::test]
    async fn replayed_history_reuses_the_web_conversation() {
        let mut cfg = AppConfig::default();
        cfg.server.api_token = String::new(); // 空 token = 仅本机不校验,测试只关心会话沿用
        let model = cfg.models.default_variant().expect("默认变体存在").id.clone();
        let backend = Arc::new(MockChatBackend::new(vec![]));
        let app = router(state_with_backend(cfg, backend.clone()));

        let first = chat(
            &app,
            serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": "现在几点"}],
            }),
        )
        .await;
        let answer = first["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_else(|| panic!("首个响应里没有答复: {first}"))
            .to_string();

        let second = chat(
            &app,
            serde_json::json!({
                "model": model,
                "messages": [
                    {"role": "user", "content": "现在几点"},
                    {"role": "assistant", "content": answer},
                    {"role": "user", "content": "那明天呢"},
                ],
            }),
        )
        .await;

        assert_eq!(
            backend.injections().len(),
            1,
            "同一网页对话只注入一次强提示词"
        );
        let payload = backend.sent().last().cloned().expect("至少发过一条消息");
        assert!(payload.contains("【本轮客户端消息】\n那明天呢"), "{payload}");
        assert!(
            !payload.contains("工具调用代理"),
            "续轮不能带强提示词:{payload}"
        );
        assert_eq!(
            first["ds_webagent"]["session_id"], second["ds_webagent"]["session_id"],
            "两轮应当落在同一条会话上"
        );
        assert_eq!(
            first["ds_webagent"]["conversation_id"], second["ds_webagent"]["conversation_id"],
            "两轮应当落在同一个网页对话上"
        );

        // 客户端开了新聊天(锚点变了、历史里没有我们的答复):新对话 + 重新注入一次
        let third = chat(
            &app,
            serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": "另起一个话题"}],
            }),
        )
        .await;
        assert_eq!(backend.injections().len(), 2, "新聊天要重新注入一次强提示词");
        assert_ne!(
            third["ds_webagent"]["session_id"], second["ds_webagent"]["session_id"]
        );
    }

    fn state() -> AppState {
        state_with(AppConfig::default())
    }

    #[test]
    fn unknown_model_is_reported_as_not_found() {
        let state = state();
        let err = resolve_model(&state, Some("gpt-4o")).unwrap_err();
        assert!(matches!(err, AppError::ModelNotFound(_)), "{err}");
        assert_eq!(err.status(), StatusCode::NOT_FOUND);
        assert_eq!(err.error_type(), "invalid_request_error");
        assert_eq!(err.code(), "model_not_found");
        assert_eq!(err.param(), Some("model"));
        // 省略 model 时用配置里的默认变体
        assert!(resolve_model(&state, None).is_ok());
    }

    #[test]
    fn tool_choice_selects_declared_tools() {
        let request = |choice: Option<Value>| OpenAiChatRequest {
            model: None,
            messages: Vec::new(),
            stream: false,
            stream_options: None,
            tools: Some(json!([
                {"type": "function", "function": {"name": "get_time"}},
                {"type": "function", "function": {"name": "get_weather"}},
            ])),
            tool_choice: choice,
            session_id: None,
            max_rounds: None,
        };
        let names = |tools: Vec<ClientToolSpec>| {
            tools.into_iter().map(|t| t.name).collect::<Vec<_>>()
        };

        assert_eq!(names(select_client_tools(&request(None)).unwrap()).len(), 2);
        assert!(select_client_tools(&request(Some(json!("none")))).unwrap().is_empty());
        assert_eq!(
            names(
                select_client_tools(&request(Some(json!({
                    "type": "function",
                    "function": {"name": "get_weather"}
                }))))
                .unwrap()
            ),
            vec!["get_weather".to_string()]
        );
        // 指定了没声明的函数:按 OpenAI 的做法直接报参数错误
        let err = select_client_tools(&request(Some(json!({
            "type": "function",
            "function": {"name": "nope"}
        }))))
        .unwrap_err();
        assert_eq!(err.param(), Some("tool_choice"));
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    /// 用户明确要求:`/v1/stats` 不带 token 也要能打开;其余端点该拦还得拦。
    #[tokio::test]
    async fn stats_needs_no_token_but_others_do() {
        use axum::body::Body;
        use axum::http::Request as HttpRequest;
        use tower::ServiceExt;

        let mut cfg = AppConfig::default();
        cfg.server.api_token = "secret".into();
        let app = router(state_with(cfg));

        let stats = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/v1/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("/v1/stats 应当无需 token 就能打开");
        assert_eq!(stats.status(), StatusCode::OK);

        let sessions = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/v1/sessions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("受保护端点应当有响应");
        assert_eq!(sessions.status(), StatusCode::UNAUTHORIZED);
    }
}
