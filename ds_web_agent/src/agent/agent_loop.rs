//! AgentLoop:一轮一轮地"发消息 → 解析 JSON → 执行工具 → 回填结果",直到 `final` 或超限。
//!
//! 不变量(改动本文件时必须保持):
//! 1. **同一对话内强提示词默认只注入一次**——`start()` 和 `rotate()` 的首条消息带它,
//!    普通轮次走 `send()` 只发载荷;唯一例外是**解析失败后的重试**:那一条会把完整强提示词
//!    连同修正提示一起重发(模型被客户端格式带偏时,只补一句修正往往继续跑偏);
//! 2. 轮换的顺序必须是"先在旧对话里总结,再开新对话",顺序反了就丢上下文;
//! 3. 每一条发给网页的消息都要记账(第 12 节的用量统计按"网页对话实际消耗"算)。

use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::agent::json_parser::{correction_message, parse_action, ModelAction};
use crate::agent::prompt_builder::{describe_model_mode, PromptBuilder, PromptContext};
use crate::config::{AppConfig, ModelVariant};
use crate::conversation::{ChatBackend, ContextDecision, ContextManager, Reply};
use crate::error::{AppError, Result};
use crate::storage::{RoundRecord, Store, ToolCallRecord};
use crate::tools::ToolRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// 模型要求调用工具,并且结果已回填
    ToolCall,
    /// 模型给出最终答案
    Final,
    /// 上下文轮换(换新对话)
    Rotation,
}

/// 一次 API 请求里的一个可追溯步骤。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRecord {
    pub round: usize,
    pub kind: StepKind,
    /// 模型这一轮输出的原始 JSON
    pub model_json: Option<Value>,
    /// `{"tool":..,"ok":..,"data":..}` 或失败时的 `{"tool":..,"ok":false,"error":..}`
    pub tool_result: Option<Value>,
    pub tool_ok: Option<bool>,
    pub duration_ms: u64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationRecord {
    pub round: usize,
    pub from_conversation_id: String,
    pub to_conversation_id: String,
    /// 触发原因(`context_full` / `too_many_rounds`)
    pub reason: String,
    pub summary_chars: usize,
}

/// 一次请求的用量,拆成"网页实际消耗"和"有效用量"两套口径。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunUsage {
    /// 总:强提示词 + 载荷 + 模型返回 JSON + 模型思考 + 工具结果回填
    pub consumed_chars: usize,
    pub prompt_chars: usize,
    pub reply_chars: usize,
    pub thinking_chars: usize,
    pub tool_result_chars: usize,
    /// 有效:API 入参 + 最终返回给客户端的内容
    pub effective_chars: usize,
    pub rounds: usize,
    pub tool_calls: usize,
}

impl RunUsage {
    fn record(&mut self, payload: &str, reply: &Reply) {
        let payload_chars = payload.chars().count();
        self.prompt_chars += payload_chars;
        self.reply_chars += reply.chars;
        self.thinking_chars += reply.thinking_chars();
        self.consumed_chars += payload_chars + reply.full_chars;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutcome {
    pub session_id: String,
    pub conversation_id: String,
    pub content: String,
    pub steps: Vec<StepRecord>,
    pub rotations: Vec<RotationRecord>,
    pub rounds: usize,
    pub usage: RunUsage,
    /// 本会话是否已注入强提示词(正常必定为 true,失败链路可用来定位)
    pub prompt_injected: bool,
    /// 本次请求允许调用的工具(已与注册表、全局白名单求交)
    pub tool_whitelist: Vec<String>,
    /// 客户端工具模式下,模型请求交给客户端执行的调用(本地不执行)
    #[serde(default)]
    pub tool_call: Option<ClientToolCall>,
}

/// 客户端声明的工具(OpenAI `tools`):名字、描述、参数 schema 全部来自请求。
///
/// 这类工具本地既不注册也不执行:桥梁只把声明注入强提示词,模型请求时原样交还客户端,
/// 由客户端执行后再以 `role: "tool"` 把结果送回来。DeepSeek 网页本身没有工具能力。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientToolSpec {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parameters: Value,
}

/// 交还给客户端的工具调用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// 流式接口用的过程事件。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    Started {
        session_id: String,
        conversation_id: String,
        model: String,
    },
    Round {
        round: usize,
    },
    ToolCall {
        round: usize,
        tool: String,
        ok: bool,
        duration_ms: u64,
    },
    Rotation {
        round: usize,
        from: String,
        to: String,
        reason: String,
    },
    Final {
        round: usize,
        chars: usize,
    },
}

pub struct AgentRun {
    pub session_id: Option<String>,
    pub input: String,
    pub tools: Vec<String>,
    /// 非空表示客户端工具模式:模型请求工具时交还客户端,而不是本地执行
    pub client_tools: Vec<ClientToolSpec>,
    pub max_rounds: usize,
    pub model: ModelVariant,
    /// SSE 模式下用来推送过程事件
    pub events: Option<tokio::sync::mpsc::UnboundedSender<AgentEvent>>,
    /// 外部取消(客户端断开)
    pub cancel: Option<tokio::sync::watch::Receiver<bool>>,
    /// 需要上传的文件路径列表(在发送消息前上传到当前对话)
    pub file_paths: Vec<String>,
}

pub struct AgentLoop {
    backend: Arc<dyn ChatBackend>,
    registry: Arc<ToolRegistry>,
    prompt: PromptBuilder,
    cfg: Arc<AppConfig>,
    store: Arc<Store>,
    context: ContextManager,
    /// 网页对话是有状态资源:低频串行,避免并发发消息互相踩。
    gate: tokio::sync::Mutex<()>,
}

impl AgentLoop {
    pub fn new(
        backend: Arc<dyn ChatBackend>,
        registry: Arc<ToolRegistry>,
        cfg: Arc<AppConfig>,
        store: Arc<Store>,
    ) -> Self {
        let prompt = PromptBuilder::new(&cfg.agent);
        let context = ContextManager::new(&cfg);
        Self {
            backend,
            registry,
            prompt,
            cfg,
            store,
            context,
            gate: tokio::sync::Mutex::new(()),
        }
    }

    fn emit(&self, run: &AgentRun, event: AgentEvent) {
        if let Some(tx) = &run.events {
            // 接收端断开不是错误:客户端只是不关心过程了
            let _ = tx.send(event);
        }
    }

    fn check_cancel(&self, run: &AgentRun) -> Result<()> {
        let cancelled = run
            .cancel
            .as_ref()
            .map(|rx| *rx.borrow())
            .unwrap_or(false);
        if cancelled {
            Err(AppError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// 沿用已有会话时,把客户端这次的内容包成一条普通消息。
    fn user_turn(&self, input: &str) -> String {
        format!("【用户消息】\n{}\n\n请只输出一个 JSON 对象。", input.trim())
    }

    fn environment_hint(&self) -> String {
        format!(
            "宿主:{} {};工作目录:{};本地时间:{};桥接端口:{}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "未知".into()),
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z"),
            self.cfg.server.port
        )
    }

    fn new_session_id() -> String {
        format!("sess-{}", uuid::Uuid::new_v4().simple())
    }

    fn round_record(
        &self,
        session_id: &str,
        conversation_id: &str,
        round: usize,
        kind: &str,
        payload: &str,
        reply: &Reply,
    ) -> RoundRecord {
        RoundRecord {
            session_id: session_id.to_string(),
            conversation_id: conversation_id.to_string(),
            round,
            kind: kind.to_string(),
            payload_chars: payload.chars().count(),
            reply_chars: reply.chars,
            thinking_chars: reply.thinking_chars(),
            model_json: None,
            parsed_type: None,
        }
    }

    /// 当前任务状态说明,轮换时交给新对话。
    fn task_state(&self, round: usize, input: &str, tool_names: &[String]) -> String {
        let input_digest: String = input.chars().take(400).collect();
        format!(
            "原始任务:{};当前进行到第 {round} 轮(上限 {}) ;可用工具:{:?};继续按 JSON 协议输出。",
            input_digest.replace('\n', " "),
            self.cfg.agent.max_rounds,
            tool_names
        )
    }

    /// 上下文接近上限:先在**旧对话**里总结,再开新对话,首条消息里带回摘要 + 状态 + 待处理载荷。
    async fn rotate(
        &self,
        run: &AgentRun,
        session_id: &str,
        conversation_id: &str,
        round: usize,
        prompt_block: &str,
        reason_tag: &str,
        pending_payload: &str,
        tool_names: &[String],
        original_input: &str,
    ) -> Result<(Reply, RotationRecord)> {
        // 1) 让快满的这个对话自己产出摘要(必须发生在点「新对话」之前)
        let summary_request = self.context.summary_request();
        let summary_reply = self.backend.send(session_id, &summary_request, &run.model).await?;
        self.store.record_round(&self.round_record(
            session_id,
            conversation_id,
            round,
            "summary",
            &summary_request,
            &summary_reply,
        ));
        let summary = extract_summary(&summary_reply.text, self.context.summary_max_chars());
        info!(
            round,
            summary_chars = summary.chars().count(),
            "已在旧对话内生成交接摘要"
        );

        // 2) 新对话首条消息 = 强提示词 + 摘要 + 当前任务状态 + 待处理载荷
        let task_state = self.task_state(round, original_input, tool_names);
        let mut first_message = prompt_block.to_string();
        first_message.push_str(&self.context.resume_block(&summary, &task_state));
        first_message.push_str("\n\n【待处理消息】\n");
        first_message.push_str(pending_payload.trim());
        first_message.push_str("\n\n请接着这条消息继续决策,只输出一个 JSON 对象。");

        let (new_conversation_id, reply) = self
            .backend
            .rotate(session_id, &first_message, &run.model)
            .await?;

        Ok((
            reply,
            RotationRecord {
                round,
                from_conversation_id: conversation_id.to_string(),
                to_conversation_id: new_conversation_id,
                reason: reason_tag.to_string(),
                summary_chars: summary.chars().count(),
            },
        ))
    }

    pub async fn run(&self, run: AgentRun) -> Result<AgentOutcome> {
        let input = run.input.trim().to_string();
        if input.is_empty() {
            return Err(AppError::BadRequest("input 不能为空".into()));
        }

        // 工具集有两种来源:
        // - 本地模式:注册表 + `[tools].enabled` 白名单,由本进程执行;
        // - 客户端模式(OpenAI 协议):工具由请求声明、由客户端执行,这里只把它们如实
        //   注入强提示词,模型请求工具时原样交还,绝不本地执行。
        let client_driven = !run.client_tools.is_empty();
        let tools_json = if client_driven {
            Value::Array(
                run.client_tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                            "source": "client",
                        })
                    })
                    .collect(),
            )
        } else {
            let specs = self.registry.specs_for(&run.tools)?;
            serde_json::to_value(&specs).unwrap_or_else(|_| Value::Array(vec![]))
        };
        let allowed: Vec<String> = if client_driven {
            run.client_tools.iter().map(|t| t.name.clone()).collect()
        } else {
            tools_json
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|s| s.get("name").and_then(Value::as_str))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };

        // 网页对话是串行资源:整个请求期间独占,避免两个请求互相把消息发进同一个对话
        let _gate = self.gate.lock().await;

        let ctx = PromptContext {
            tools: tools_json,
            model_mode: describe_model_mode(&run.model),
            environment: self.environment_hint(),
        };
        let prompt_block = self.prompt.render(&ctx);

        let mut usage = RunUsage::default();
        let mut steps: Vec<StepRecord> = Vec::new();
        let mut rotations: Vec<RotationRecord> = Vec::new();

        // ---------- 会话准备 ----------
        let mut new_run: Option<(String, String, Reply)> = None; // (session_id, conversation_id, reply)
        let (session_id, mut pending) = match run.session_id.as_deref() {
            Some(sid) if self.backend.state(sid).await.is_ok() => {
                debug!(session_id = sid, "沿用已有会话,不重复注入强提示词");
                (sid.to_string(), Some(self.user_turn(&input)))
            }
            existing => {
                let sid = match existing {
                    Some(sid) => sid.to_string(),
                    None => Self::new_session_id(),
                };
                let first_message = self.prompt.compose_first(&ctx, &input);
                let (conversation_id, reply) =
                    self.backend.start(&sid, &first_message, &run.model).await?;
                self.backend.remember_prompt(&sid, &prompt_block).await?;
                usage.record(&first_message, &reply);
                self.store.record_session_created(&sid, &run.model.id);
                self.store.record_request(&sid, &run.model.id, true);
                self.store.record_conversation_created(
                    &conversation_id,
                    &sid,
                    &run.model.id,
                    &run.model.mode_name(),
                );
                self.store.record_round(&self.round_record(
                    &sid,
                    &conversation_id,
                    1,
                    "protocol",
                    &first_message,
                    &reply,
                ));
                new_run = Some((sid.clone(), conversation_id, reply));
                (sid, None)
            }
        };

        // 上传文件/图片到当前对话(在消息发送前完成)
        if !run.file_paths.is_empty() {
            for file_path in &run.file_paths {
                info!(session_id = %session_id, file = %file_path, "上传文件到对话");
                if let Err(e) = self.backend.upload_file(&session_id, file_path).await {
                    warn!(session_id = %session_id, file = %file_path, error = %e, "文件上传失败");
                    // 上传失败不阻断流程,继续发送消息
                }
            }
        }

        let mut conversation_id = match &new_run {
            Some((_, cid, _)) => cid.clone(),
            None => self.backend.conversation_id(&session_id).await?,
        };
        if run.session_id.is_some() && new_run.is_none() {
            self.store.record_request(&session_id, &run.model.id, false);
        }
        let mut next_reply = new_run.take().map(|(_, _, reply)| reply);

        self.emit(
            &run,
            AgentEvent::Started {
                session_id: session_id.clone(),
                conversation_id: conversation_id.clone(),
                model: run.model.id.clone(),
            },
        );

        // ---------- 主循环 ----------
        let mut round = 0usize;
        let mut corrections = 0usize;

        loop {
            round += 1;
            if round > run.max_rounds {
                self.store.record_error(
                    Some(&session_id),
                    "max_rounds_exceeded",
                    &format!("超过最大轮次 {}", run.max_rounds),
                    &format!("conversation_id={conversation_id}"),
                );
                return Err(AppError::MaxRoundsExceeded(run.max_rounds));
            }
            self.check_cancel(&run)?;
            usage.rounds = round;
            self.emit(&run, AgentEvent::Round { round });

            // 本轮回复:可能来自 start()/rotate(),也可能现在才发出去
            let reply = match next_reply.take() {
                Some(reply) => reply,
                None => {
                    let payload = pending.take().ok_or_else(|| {
                        AppError::Internal("内部错误:没有待发送内容却没有本轮回复".into())
                    })?;
                    let state = self.backend.state(&session_id).await?;
                    match self
                        .context
                        .decide(state.context_chars, state.rounds_in_conversation)
                    {
                        ContextDecision::Continue => {
                            let reply = self
                                .backend
                                .send(&session_id, &payload, &run.model)
                                .await?;
                            usage.record(&payload, &reply);
                            self.store.record_round(&self.round_record(
                                &session_id,
                                &conversation_id,
                                round,
                                "payload",
                                &payload,
                                &reply,
                            ));
                            reply
                        }
                        ContextDecision::Rotate { reason } => {
                            let tag = reason_tag(&reason);
                            warn!(
                                round,
                                reason = %tag,
                                context_chars = state.context_chars,
                                "上下文接近上限,准备轮换到新对话"
                            );
                            let (reply, rotation) = self
                                .rotate(
                                    &run,
                                    &session_id,
                                    &conversation_id,
                                    round,
                                    &prompt_block,
                                    &tag,
                                    &payload,
                                    &allowed,
                                    &input,
                                )
                                .await?;
                            usage.record(&payload, &reply);
                            self.store.record_rotation(&session_id);
                            self.store.record_conversation_created(
                                &rotation.to_conversation_id,
                                &session_id,
                                &run.model.id,
                                &run.model.mode_name(),
                            );
                            self.emit(
                                &run,
                                AgentEvent::Rotation {
                                    round,
                                    from: rotation.from_conversation_id.clone(),
                                    to: rotation.to_conversation_id.clone(),
                                    reason: tag.clone(),
                                },
                            );
                            steps.push(StepRecord {
                                round,
                                kind: StepKind::Rotation,
                                model_json: None,
                                tool_result: None,
                                tool_ok: None,
                                duration_ms: reply.elapsed_ms,
                                note: Some(format!(
                                    "上下文轮换({tag}):{} -> {}",
                                    rotation.from_conversation_id, rotation.to_conversation_id
                                )),
                            });
                            conversation_id = rotation.to_conversation_id.clone();
                            rotations.push(rotation);
                            reply
                        }
                    }
                }
            };

            self.check_cancel(&run)?;

            // ---------- 解析模型输出 ----------
            match parse_action(&reply.text) {
                Ok(parsed) => {
                    corrections = 0;
                    match parsed.action {
                        ModelAction::ToolCall { tool, arguments } => {
                            if client_driven {
                                // 客户端模式:网页模型只负责决定调用什么,执行由客户端完成。
                                // 立刻把调用交还,不本地执行、不等待结果。
                                let call = ClientToolCall {
                                    id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                                    name: tool,
                                    arguments,
                                };
                                usage.effective_chars = input.chars().count();
                                steps.push(StepRecord {
                                    round,
                                    kind: StepKind::ToolCall,
                                    model_json: Some(parsed.model_json),
                                    tool_result: None,
                                    tool_ok: None,
                                    duration_ms: reply.elapsed_ms,
                                    note: Some("交还客户端执行".into()),
                                });
                                let state = self.backend.state(&session_id).await?;
                                info!(
                                    session_id = %session_id,
                                    conversation_id = %conversation_id,
                                    round,
                                    tool = %call.name,
                                    "把工具调用交还客户端"
                                );
                                self.emit(
                                    &run,
                                    AgentEvent::ToolCall {
                                        round,
                                        tool: call.name.clone(),
                                        ok: true,
                                        duration_ms: reply.elapsed_ms,
                                    },
                                );
                                return Ok(AgentOutcome {
                                    session_id,
                                    conversation_id,
                                    content: String::new(),
                                    tool_call: Some(call),
                                    steps,
                                    rotations,
                                    rounds: round,
                                    usage,
                                    prompt_injected: state.prompt_injected,
                                    tool_whitelist: allowed,
                                });
                            }
                            if !allowed.iter().any(|n| n == &tool) {
                                // 不在本次允许列表里:当成工具错误回填,让模型改用允许的工具
                                corrections += 1;
                                let message = format!(
                                    "工具 `{tool}` 不在本次允许调用的列表内,允许的是 {allowed:?}"
                                );
                                let payload = PromptBuilder::compose_tool_error(&tool, &message);
                                self.store.record_tool_call(&ToolCallRecord {
                                    session_id: session_id.clone(),
                                    conversation_id: conversation_id.clone(),
                                    round,
                                    tool: tool.clone(),
                                    ok: false,
                                    duration_ms: 0,
                                    args: Some(arguments.to_string()),
                                    result: None,
                                    error: Some(message.clone()),
                                });
                                steps.push(StepRecord {
                                    round,
                                    kind: StepKind::ToolCall,
                                    model_json: Some(parsed.model_json),
                                    tool_result: Some(serde_json::json!({
                                        "tool": tool,
                                        "ok": false,
                                        "error": message,
                                    })),
                                    tool_ok: Some(false),
                                    duration_ms: 0,
                                    note: Some("工具不在本次白名单内".into()),
                                });
                                if corrections > self.cfg.agent.max_json_retries {
                                    return Err(AppError::ToolNotAllowed(format!(
                                        "{tool}(模型连续请求白名单外的工具)"
                                    )));
                                }
                                pending = Some(payload);
                                continue;
                            }

                            let started = Instant::now();
                            let outcome = self.registry.execute(&tool, arguments.clone()).await;
                            let duration_ms = started.elapsed().as_millis() as u64;
                            let (ok, data, error) = match outcome {
                                Ok(value) => (true, value, None),
                                Err(e) => (false, Value::Null, Some(e.to_string())),
                            };

                            // 工具失败也要回填:模型需要看到 ok=false 和原因
                            let payload = match &error {
                                Some(err) => PromptBuilder::compose_tool_error(&tool, err),
                                None => PromptBuilder::compose_tool_result(&tool, true, &data),
                            };
                            usage.tool_calls += 1;
                            usage.tool_result_chars += payload.chars().count();
                            self.store.record_tool_result_chars(payload.chars().count());
                            self.store.record_tool_call(&ToolCallRecord {
                                session_id: session_id.clone(),
                                conversation_id: conversation_id.clone(),
                                round,
                                tool: tool.clone(),
                                ok,
                                duration_ms,
                                args: Some(arguments.to_string()),
                                result: ok.then(|| data.to_string()),
                                error: error.clone(),
                            });
                            info!(round, tool = %tool, ok, duration_ms, "工具执行完成");
                            self.emit(
                                &run,
                                AgentEvent::ToolCall {
                                    round,
                                    tool: tool.clone(),
                                    ok,
                                    duration_ms,
                                },
                            );

                            steps.push(StepRecord {
                                round,
                                kind: StepKind::ToolCall,
                                model_json: Some(parsed.model_json),
                                tool_result: Some(serde_json::json!({
                                    "tool": tool,
                                    "ok": ok,
                                    "data": data,
                                    "error": error,
                                })),
                                tool_ok: Some(ok),
                                duration_ms,
                                note: None,
                            });
                            pending = Some(payload);
                        }
                        ModelAction::Final { content } => {
                            steps.push(StepRecord {
                                round,
                                kind: StepKind::Final,
                                model_json: Some(parsed.model_json),
                                tool_result: None,
                                tool_ok: None,
                                duration_ms: reply.elapsed_ms,
                                note: None,
                            });
                            usage.effective_chars =
                                input.chars().count() + content.chars().count();
                            self.store
                                .record_effective(input.chars().count(), content.chars().count());
                            let state = self.backend.state(&session_id).await?;
                            self.emit(
                                &run,
                                AgentEvent::Final {
                                    round,
                                    chars: content.chars().count(),
                                },
                            );
                            info!(
                                session_id = %session_id,
                                conversation_id = %conversation_id,
                                rounds = round,
                                "请求完成"
                            );
                            return Ok(AgentOutcome {
                                session_id,
                                conversation_id,
                                content,
                                tool_call: None,
                                steps,
                                rotations,
                                rounds: round,
                                usage,
                                prompt_injected: state.prompt_injected,
                                tool_whitelist: allowed,
                            });
                        }
                    }
                }
                Err(e) => {
                    corrections += 1;
                    let message = e.to_string();
                    let excerpt: String = reply.text.chars().take(500).collect();
                    self.store
                        .record_error(Some(&session_id), e.code(), &message, &excerpt);
                    warn!(round, corrections, error = %message, "模型输出无法解析");
                    if corrections > self.cfg.agent.max_json_retries {
                        return Err(AppError::JsonParse(format!(
                            "连续 {corrections} 次无法解析模型输出,最后一次错误:{message}"
                        )));
                    }
                    // 重新注入**完整强提示词** + 修正提示:解析失败多半是模型被客户端的格式要求带偏
                    // (例如把客户端要的 JSON 原始塞进 content 却没转义),只补一句"重新输出 JSON"很容易继续跑偏。
                    // 这是"强提示词只注入一次"的唯一例外,见文件头不变量 1。
                    let payload = format!(
                        "{prompt_block}\n\n{correction}",
                        correction = correction_message(&message, &reply.text)
                    );
                    self.store.record_round(&self.round_record(
                        &session_id,
                        &conversation_id,
                        round,
                        "correction",
                        &payload,
                        &reply,
                    ));
                    pending = Some(payload);
                }
            }
        }
    }
}

fn reason_tag(reason: &crate::conversation::RotateReason) -> String {
    match reason {
        crate::conversation::RotateReason::ContextFull { .. } => "context_full".to_string(),
        crate::conversation::RotateReason::TooManyRounds { .. } => "too_many_rounds".to_string(),
    }
}

/// 摘要解析:协议里摘要就放在 `{"type":"final","content":"..."}` 里;
/// 万一模型没按协议走,就退化成截断原文,不让轮换因为格式问题失败。
fn extract_summary(text: &str, max_chars: usize) -> String {
    let candidate = match parse_action(text) {
        Ok(parsed) => match parsed.action {
            ModelAction::Final { content } => content,
            ModelAction::ToolCall { .. } => text.to_string(),
        },
        Err(_) => text.to_string(),
    };
    let trimmed = candidate.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let head: String = trimmed.chars().take(max_chars).collect();
    format!("{head}…(摘要已按配置截断)")
}
