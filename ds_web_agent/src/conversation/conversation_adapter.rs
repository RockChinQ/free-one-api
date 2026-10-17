//! `ChatBackend` 的真实浏览器实现:把网页操作(/新建对话、输入、发送、等待、提取)串成事务。
//!
//! 这一层是"DeepSeek 网页对话"和"Agent 协议"的接缝,因此承担三件事:
//! 1. **不改写业务含义**:只做收发,不知道 JSON 协议是什么;
//! 2. **每次发送都确认生效**:输入框清空或进入生成态才算发出去,否则按配置重试一次;
//! 3. **失败要可诊断**:超时/状态异常时带上 URL、标题和截图路径,便于只改 config.toml 修复。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::browser::{BrowserAdapter, LoginState};
use crate::config::{AppConfig, ModelVariant};
use crate::conversation::{ChatBackend, ClientTurn, Reply, SessionRecord, SessionState};
use crate::error::{AppError, Result};

/// 在候选会话里挑出该沿用的那个(纯函数,便于用单元测试锁死判据)。
///
/// 三个条件缺一不可:
/// 1. 模型一致——搜索/思考开关是按网页对话生效的,换模型就应当新开对话;
/// 2. 客户端历史里回放着我们上一轮的答复——要么整段前缀都对得上(客户端按原样回放),
///    要么历史以那一行答复结尾;
/// 3. 还得确认就是这一条对话:历史接得上,或者锚点(第一条 user 消息)一致。
///    对不上宁可新开对话重新注入,也不要把消息串进别人的对话里。
pub(crate) fn affinity_match<'a>(
    sessions: impl Iterator<Item = &'a SessionState>,
    anchor: &str,
    transcript: &str,
    model_id: &str,
) -> Option<&'a SessionState> {
    let tail = transcript.trim_end();
    let anchor = anchor.trim();
    let mut best: Option<(&SessionState, (u8, u8))> = None;

    for state in sessions.filter(|s| s.model_id == model_id) {
        let client = &state.client;
        let answer = client.answer_line.trim();
        let known = client.known_prefix();
        let prefix_aligned = !known.is_empty() && tail.starts_with(&known);
        let answer_aligned = !answer.is_empty() && tail.ends_with(answer);
        if !prefix_aligned && !answer_aligned {
            continue;
        }
        let anchor_aligned = !anchor.is_empty() && client.anchor == anchor;
        if !prefix_aligned && !anchor_aligned {
            continue;
        }
        let score = (prefix_aligned as u8, anchor_aligned as u8);
        let better = match best {
            None => true,
            Some((current, current_score)) => {
                score > current_score
                    || (score == current_score && state.last_active > current.last_active)
            }
        };
        if better {
            best = Some((state, score));
        }
    }

    best.map(|(state, _)| state)
}

pub struct BrowserChatBackend {
    browser: Arc<dyn BrowserAdapter>,
    cfg: Arc<AppConfig>,
    sessions: tokio::sync::Mutex<HashMap<String, SessionState>>,
}

impl BrowserChatBackend {
    pub fn new(browser: Arc<dyn BrowserAdapter>, cfg: Arc<AppConfig>) -> Self {
        Self {
            browser,
            cfg,
            sessions: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.cfg.conversation.poll_interval_ms.max(100))
    }

    fn new_conversation_id() -> String {
        let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let short = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
        format!("ds-{stamp}-{short}")
    }

    /// 每次收发前确认"页面可用":登录失效返回 NotLoggedIn,页面状态不明返回可诊断错误。
    async fn guard_ready(&self) -> Result<()> {
        let wait = Duration::from_secs(self.cfg.browser.login_poll_secs.max(1) + 4);
        match self.browser.wait_for_login(Some(wait)).await? {
            LoginState::LoggedIn => Ok(()),
            LoginState::WaitingLogin => Err(AppError::NotLoggedIn),
            // Starting/Failed 都表示"说不清楚页面现在是什么状态",不能当成已登录
            LoginState::Starting | LoginState::Failed => Err(AppError::Browser(format!(
                "页面状态异常(可能仍在加载,或 [selectors] 已漂移):{}",
                self.browser.describe_page().await
            ))),
        }
    }

    /// 输入并发送;只有当"发送确实生效"才返回 Ok,否则按 `send_retry` 重试一次。
    ///
    /// "生效"的判据必须包含"页面已经出现新回复":输入框内容还在,并不等于消息没发出去。
    /// 大载荷或页面正卡在切换对话时,会出现"消息已提交、回复已渲染,输入框内容却还在"的中间态;
    /// 只看输入框会把它判成失败并重发同一段文本,于是模型对同一内容回答两次,上下文与用量都被污染。
    /// `before_count` 由调用方在发送前取好,是"这条消息发出前页面已有多少条回复"的基线。
    async fn type_and_submit(&self, text: &str, before_count: usize) -> Result<()> {
        let attempts = self.cfg.conversation.send_retry.max(0) + 1;
        let mut last_error: Option<AppError> = None;

        for attempt in 1..=attempts {
            // 残留文本会被拼到新消息前面,先清空
            let existing = self.browser.input_value().await.unwrap_or_default();
            if !existing.trim().is_empty() {
                self.browser.clear_input().await?;
            }

            if let Err(e) = self.browser.type_message(text).await {
                last_error = Some(e);
                continue;
            }

            let typed = self.browser.input_value().await.unwrap_or_default();
            if typed.trim().is_empty() {
                last_error = Some(AppError::SendFailed(
                    "输入框没有写入任何内容(焦点或选择器可能有问题)".into(),
                ));
                continue;
            }

            if let Err(e) = self.browser.submit().await {
                last_error = Some(e);
                continue;
            }

            // 发送生效的判据(满足其一即可):进入生成态 / 输入框被清空 / 页面出现了新的 assistant 回复
            let deadline = Instant::now()
                + Duration::from_millis(self.cfg.conversation.min_wait_after_send_ms.max(500) * 2);
            loop {
                if self.browser.is_generating().await.unwrap_or(false) {
                    return Ok(());
                }
                let value = self.browser.input_value().await.unwrap_or_default();
                if value.trim().is_empty() {
                    return Ok(());
                }
                if self.browser.assistant_message_count().await.unwrap_or(0) > before_count {
                    // 模型已经开始或完成回答,说明这条消息确实提交了,绝不能重发
                    debug!(
                        attempt,
                        "输入框内容没有清空,但页面已出现新回复:判定发送生效,不重发"
                    );
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(self.poll_interval()).await;
            }

            warn!(attempt, "提交后输入框内容没有消失,消息可能没发出去");
            last_error = Some(AppError::SendFailed(
                "提交后输入框内容没有消失,消息可能没有发出".into(),
            ));
        }

        Err(last_error.unwrap_or_else(|| AppError::SendFailed("发送失败".into())))
    }

    /// 等流式结束:先等到"停止生成"消失且出现新回复,再等文本连续稳定 `stable_secs`。
    async fn wait_for_reply(&self, before_count: usize) -> Result<Reply> {
        let started = Instant::now();
        let timeout = Duration::from_secs(self.cfg.conversation.reply_timeout_secs.max(5));
        let poll = self.poll_interval();

        let mut text;
        loop {
            if started.elapsed() >= timeout {
                return Err(self.reply_timeout(started.elapsed()).await);
            }
            let generating = self.browser.is_generating().await.unwrap_or(false);
            if !generating {
                let count = self.browser.assistant_message_count().await.unwrap_or(0);
                if count > before_count {
                    let current = self.browser.last_assistant_text().await?.unwrap_or_default();
                    if !current.trim().is_empty() {
                        text = current;
                        break;
                    }
                }
            }
            tokio::time::sleep(poll).await;
        }

        // 稳定性确认:流式渲染下"停止生成"消失后正文可能还在补;文本连续 N 秒不变才算完。
        let stable = Duration::from_secs(self.cfg.conversation.stable_secs.max(1));
        let stable_deadline = Instant::now() + stable + Duration::from_secs(30);
        let mut stable_since = Instant::now();
        loop {
            if Instant::now() >= stable_deadline {
                warn!("等待文本稳定超时,按当前内容收尾");
                break;
            }
            tokio::time::sleep(poll).await;
            let generating = self.browser.is_generating().await.unwrap_or(false);
            let current = self.browser.last_assistant_text().await?.unwrap_or_default();
            if current != text {
                text = current;
                stable_since = Instant::now();
                continue;
            }
            if !generating && stable_since.elapsed() >= stable {
                break;
            }
        }

        if text.trim().is_empty() {
            return Err(AppError::EmptyReply(
                "最后一条 assistant 消息为空,可能页面结构变化导致提取失败".into(),
            ));
        }

        // 全文(含思考过程)只用于统计,拿不到就退回正文
        let full_text = self
            .browser
            .last_assistant_full_text()
            .await
            .ok()
            .flatten()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| text.clone());
        let count = self
            .browser
            .assistant_message_count()
            .await
            .unwrap_or(before_count + 1);

        Ok(Reply {
            chars: text.chars().count(),
            full_chars: full_text.chars().count().max(text.chars().count()),
            text,
            full_text,
            assistant_messages: count,
            elapsed_ms: started.elapsed().as_millis() as u64,
            stable_ms: stable_since.elapsed().as_millis() as u64,
        })
    }

    async fn reply_timeout(&self, elapsed: Duration) -> AppError {
        let shot = self
            .browser
            .screenshot(std::path::Path::new(&self.cfg.storage.screenshot_dir))
            .await;
        let shot_desc = match shot {
            Ok(path) => path.display().to_string(),
            Err(e) => format!("截图失败({e})"),
        };
        let page = self.browser.describe_page().await;
        tracing::error!(
            waited_secs = elapsed.as_secs(),
            page = %page,
            screenshot = %shot_desc,
            "等待回复超时"
        );
        AppError::ReplyTimeout(elapsed.as_secs())
    }

    /// 发送 + 等待 + 记账,`start` 与 `send` 共用的主体。
    async fn deliver(
        &self,
        session_id: &str,
        text: &str,
        is_prompt_injection: bool,
    ) -> Result<Reply> {
        self.guard_ready().await?;
        let before_count = self.browser.assistant_message_count().await?;
        self.type_and_submit(text, before_count).await?;
        let reply = self.wait_for_reply(before_count).await?;

        let mut sessions = self.sessions.lock().await;
        let state = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))?;
        state.record_sent(text, is_prompt_injection);
        state.record_received(&reply.text);
        if is_prompt_injection && !state.prompt_injected {
            // record_sent 只接受 true 时才置位,这里再确认一次不变量
            return Err(AppError::Internal(
                "内部不变量被破坏:首条消息注入后 prompt_injected 仍为 false".into(),
            ));
        }
        Ok(reply)
    }

    /// 点「新对话」并确认真的换到了一个空对话。
    ///
    /// 这一步必须校验:如果选择器漂移导致点击没生效,提示词就会被注入到**旧**对话里,
    /// 直接破坏"同一对话内只注入一次"这条硬约束。
    async fn open_blank_conversation(&self, model: &ModelVariant) -> Result<()> {
        self.browser.new_conversation().await?;
        if self.cfg.models.apply_toggles_on_new_conversation {
            // 开关按对话生效,所以每次新建对话都要重新设置
            if let Err(e) = self.browser.apply_toggles(model.search, model.think).await {
                warn!(error = %e, "设置搜索/思考开关失败,按页面当前状态继续");
            }
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let count = self.browser.assistant_message_count().await.unwrap_or(1);
            if count == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(AppError::Browser(format!(
                    "点击「新对话」后仍能看到 {count} 条历史回复,对话没有切换成功",
                )));
            }
            tokio::time::sleep(self.poll_interval()).await;
        }
    }
}

#[async_trait]
impl ChatBackend for BrowserChatBackend {
    async fn start(
        &self,
        session_id: &str,
        first_message: &str,
        model: &ModelVariant,
    ) -> Result<(String, Reply)> {
        if self.sessions.lock().await.contains_key(session_id) {
            return Err(AppError::BadRequest(format!(
                "session_id `{session_id}` 已存在:需要沿用旧会话就直接调用 /v1/chat,不要重复创建"
            )));
        }

        self.guard_ready().await?;
        self.open_blank_conversation(model).await?;

        let conversation_id = Self::new_conversation_id();
        {
            let mut sessions = self.sessions.lock().await;
            sessions.insert(
                session_id.to_string(),
                SessionState::new(session_id, &conversation_id, &model.id),
            );
        }

        // 首条消息 = 强提示词 + 用户内容,由调用方拼好;这里是唯一注入点。
        match self.deliver(session_id, first_message, true).await {
            Ok(reply) => {
                info!(
                    session_id,
                    conversation_id,
                    model = %model.id,
                    mode = model.mode_name(),
                    first_prompt_chars = first_message.chars().count(),
                    reply_chars = reply.chars,
                    "已创建新对话并完成首轮"
                );
                Ok((conversation_id, reply))
            }
            Err(e) => {
                // 首轮就失败,不要把半初始化的会话留在内存里
                self.sessions.lock().await.remove(session_id);
                Err(e)
            }
        }
    }

    async fn send(
        &self,
        session_id: &str,
        payload: &str,
        model: &ModelVariant,
    ) -> Result<Reply> {
        {
            let sessions = self.sessions.lock().await;
            let state = sessions
                .get(&session_id.to_string())
                .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))?;
            if !state.prompt_injected {
                return Err(AppError::Internal(
                    "该会话还没有注入强提示词,请先用 start() 建立会话".into(),
                ));
            }
        }

        debug!(session_id, model = %model.id, payload_chars = payload.chars().count(), "发送普通轮次");
        self.deliver(session_id, payload, false).await
    }

    async fn rotate(
        &self,
        session_id: &str,
        first_message: &str,
        model: &ModelVariant,
    ) -> Result<(String, Reply)> {
        let previous = {
            let sessions = self.sessions.lock().await;
            let state = sessions
                .get(&session_id.to_string())
                .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))?;
            state.conversation_id.clone()
        };

        self.open_blank_conversation(model).await?;
        let conversation_id = Self::new_conversation_id();

        {
            let mut sessions = self.sessions.lock().await;
            if let Some(state) = sessions.get_mut(session_id) {
                state.rotate_to(&conversation_id);
            }
        }

        match self.deliver(session_id, first_message, true).await {
            Ok(reply) => {
                info!(
                    session_id,
                    previous_conversation = %previous,
                    new_conversation = %conversation_id,
                    reply_chars = reply.chars,
                    "上下文接近上限,已轮换到新对话并重新注入强提示词"
                );
                Ok((conversation_id, reply))
            }
            Err(e) => {
                warn!(session_id, new_conversation = %conversation_id, error = %e, "轮换后的首轮失败");
                Err(e)
            }
        }
    }

    async fn upload_file(&self, session_id: &str, path: &str) -> Result<()> {
        {
            let sessions = self.sessions.lock().await;
            if !sessions.contains_key(session_id) {
                return Err(AppError::SessionNotFound(session_id.to_string()));
            }
        }
        self.guard_ready().await?;
        self.browser.upload_file(path).await
    }

    async fn remember_prompt(&self, session_id: &str, prompt_block: &str) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        let state = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))?;
        state.prompt_block = Some(prompt_block.to_string());
        Ok(())
    }

    async fn match_session(
        &self,
        anchor: &str,
        transcript: &str,
        model_id: &str,
    ) -> Result<Option<String>> {
        let sessions = self.sessions.lock().await;
        Ok(affinity_match(sessions.values(), anchor, transcript, model_id)
            .map(|state| state.session_id.clone()))
    }

    async fn client_turn(&self, session_id: &str) -> Result<ClientTurn> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(session_id)
            .map(|state| state.client.clone())
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))
    }

    async fn remember_client_turn(&self, session_id: &str, turn: ClientTurn) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        let state = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))?;
        state.record_client_turn(turn);
        Ok(())
    }

    async fn conversation_id(&self, session_id: &str) -> Result<String> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(session_id)
            .map(|s| s.conversation_id.clone())
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))
    }

    async fn context_chars(&self, session_id: &str) -> Result<usize> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(session_id)
            .map(|s| s.context_chars)
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))
    }

    async fn state(&self, session_id: &str) -> Result<SessionRecord> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(session_id)
            .map(SessionState::as_record)
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))
    }

    async fn list_sessions(&self) -> Result<Vec<SessionRecord>> {
        let sessions = self.sessions.lock().await;
        let mut out: Vec<SessionRecord> = sessions.values().map(SessionState::as_record).collect();
        out.sort_by(|a, b| b.last_active.cmp(&a.last_active));
        Ok(out)
    }

    async fn close_session(&self, session_id: &str) -> Result<()> {
        let removed = self.sessions.lock().await.remove(session_id);
        match removed {
            Some(state) => {
                info!(session_id, conversations = state.conversation_ids.len(), "会话已关闭");
                Ok(())
            }
            None => Err(AppError::SessionNotFound(session_id.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_ids_are_unique_and_traceable() {
        let a = BrowserChatBackend::new_conversation_id();
        let b = BrowserChatBackend::new_conversation_id();
        assert_ne!(a, b);
        assert!(a.starts_with("ds-"), "便于在日志里区分对话: {a}");
        assert_eq!(a.len(), "ds-".len() + 14 + 1 + 8);
    }

    #[tokio::test]
    async fn state_helpers_report_missing_session() {
        let cfg = Arc::new(AppConfig::default());
        let browser: Arc<dyn BrowserAdapter> = Arc::new(crate::testing::NoopBrowser::default());
        let backend = BrowserChatBackend::new(browser, cfg);
        assert!(backend.conversation_id("nope").await.is_err());
        assert!(backend.list_sessions().await.unwrap().is_empty());
        assert!(backend.close_session("nope").await.is_err());
    }

    /// 回归:页面已经渲染出新回复时,不能因为"输入框没清空"就把同一条消息再发一遍。
    /// 真实故障是消息其实已经发出、模型也回答了,日志里却打印"提交后输入框内容没有消失"并重复发送。
    #[tokio::test]
    async fn rendered_reply_is_not_resent() {
        let mut cfg = AppConfig::default();
        cfg.conversation.poll_interval_ms = 100;
        cfg.conversation.stable_secs = 1;
        let model = cfg.models.default_variant().expect("默认变体存在").clone();

        // 模拟真实页面的中间态:提交之后回复已渲染,但输入框内容迟迟不清空
        let browser = Arc::new(crate::testing::NoopBrowser::new(vec![
            r#"{"type":"final","content":"ok"}"#,
        ]));
        browser
            .keep_input_after_submit
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let backend = BrowserChatBackend::new(browser.clone(), Arc::new(cfg));
        let (_, reply) = backend
            .start("sess-repeat", "首条消息", &model)
            .await
            .expect("回复已经渲染,首轮必须判定为发送成功");
        assert_eq!(reply.text, r#"{"type":"final","content":"ok"}"#);
        assert_eq!(
            browser.submitted(),
            vec!["首条消息".to_string()],
            "页面已有新回复说明消息确实发出去了,不能再重发一遍"
        );
    }

    /// 造一条会话(模型与客户端可见状态给定)。
    fn session_with(id: &str, model: &str, turn: ClientTurn) -> SessionState {
        let mut state = SessionState::new(id, "cvt-1", model);
        state.record_client_turn(turn);
        state
    }

    fn turn(anchor: &str, transcript: &str, input_line: &str, answer: &str) -> ClientTurn {
        ClientTurn {
            anchor: anchor.into(),
            transcript: transcript.into(),
            input_line: input_line.into(),
            answer_line: answer.into(),
        }
    }

    /// 沿用判据:同模型 + 客户端历史里回放着我们的答复 + (整段前缀对齐或锚点一致)。
    #[test]
    fn affinity_requires_replayed_answer_and_matching_anchor() {
        let states = [
            session_with(
                "sess-a",
                "ds-web",
                turn("北京天气怎么样", "", "user: 北京天气怎么样", "assistant: 晴"),
            ),
            session_with(
                "sess-b",
                "ds-web",
                turn("另一件事", "", "user: 另一件事", "assistant: 好的"),
            ),
        ];
        let hit = |anchor: &str, transcript: &str, model: &str| {
            affinity_match(states.iter(), anchor, transcript, model).map(|s| s.session_id.clone())
        };

        assert_eq!(
            hit(
                "北京天气怎么样",
                "user: 北京天气怎么样\nassistant: 晴",
                "ds-web"
            ),
            Some("sess-a".to_string()),
            "回放了我们上一轮往返的请求应当沿用同一对话"
        );
        assert_eq!(
            hit("北京天气怎么样", "…\nassistant: 晴", "ds-web-search"),
            None,
            "换了模型:搜索/思考开关是按对话生效的,应该新开对话"
        );
        assert_eq!(
            hit("全新的问题", "user: 全新的问题", "ds-web"),
            None,
            "历史里没有回放我们的答复(客户端开了新聊天):应当新建对话"
        );
    }

    /// 两条会话的答复文本相同时,靠"整段前缀对齐"选出真正的那条。
    #[test]
    fn affinity_prefers_the_session_whose_history_lines_up() {
        let states = [
            session_with(
                "sess-old",
                "ds-web",
                turn("重复的问题", "", "user: 重复的问题", "assistant: ok"),
            ),
            session_with(
                "sess-new",
                "ds-web",
                turn(
                    "重复的问题",
                    "user: 重复的问题\nassistant: ok",
                    "user: 又一遍",
                    "assistant: ok",
                ),
            ),
        ];
        let hit = affinity_match(
            states.iter(),
            "重复的问题",
            "user: 重复的问题\nassistant: ok",
            "ds-web",
        );
        assert_eq!(hit.map(|s| s.session_id.as_str()), Some("sess-old"));
    }

    /// 客户端裁剪/改写过历史后,前缀与锚点都对不上:宁可选不中(新建对话),不能串到别人的对话里。
    #[test]
    fn affinity_declines_when_history_does_not_line_up() {
        let states = [session_with(
            "sess-a",
            "ds-web",
            turn(
                "很早以前的问题",
                "user: 很早以前的问题\nassistant: 好",
                "user: 追问",
                "assistant: 好",
            ),
        )];
        let hit = affinity_match(
            states.iter(),
            "被裁剪后的新开头",
            "user: 被裁剪后的新开头\nassistant: 好",
            "ds-web",
        );
        assert!(hit.is_none(), "对不上就该新建对话重新注入强提示词");
    }
}
