//! 测试支撑物:假浏览器、假对话后端,以及一个本地 mock 页面。
//!
//! 这些类型放在 lib 里(而不是 `#[cfg(test)]`)是为了让 `tests/` 下的集成测试也能复用;
//! 生产代码路径不会构造它们。

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::browser::{BrowserAdapter, LoginState};
use crate::config::ModelVariant;
use crate::conversation::{ChatBackend, Reply, SessionRecord};
use crate::error::{AppError, Result};

fn make_reply(text: &str, thinking_chars: usize) -> Reply {
    let thinking: String = "思".repeat(thinking_chars);
    let full = format!("{thinking}{text}");
    Reply {
        text: text.to_string(),
        full_text: full.clone(),
        chars: text.chars().count(),
        full_chars: full.chars().count(),
        assistant_messages: 1,
        elapsed_ms: 5,
        stable_ms: 5,
    }
}

// ------------------------------------------------------------------ 假浏览器

/// 一个纯内存的 `BrowserAdapter`:不开浏览器,但把"发出去的消息/收到的回复"记下来,
/// 用来测上层的事务逻辑(发送校验、计数、提取)。
#[derive(Default)]
pub struct NoopBrowser {
    pub login: Mutex<Option<LoginState>>,
    pub buffer: Mutex<String>,
    pub submitted: Mutex<Vec<String>>,
    pub replies: Mutex<VecDeque<String>>,
    pub assistant_count: AtomicUsize,
    pub last_reply: Mutex<Option<String>>,
    pub conversations: AtomicUsize,
    /// 模拟"回复已经渲染、输入框内容却迟迟不清空"的真实中间态:置位后 `submit` 故意保留输入框内容,
    /// 用于验证发送生效判据不会因此误判成失败并重发。
    pub keep_input_after_submit: AtomicBool,
}

impl NoopBrowser {
    pub fn new(replies: Vec<&str>) -> Self {
        let browser = Self::default();
        *browser.replies.lock().unwrap() = replies.into_iter().map(str::to_string).collect();
        browser
    }

    pub fn submitted(&self) -> Vec<String> {
        self.submitted.lock().unwrap().clone()
    }

    fn next_reply(&self) -> String {
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| "{\"type\":\"final\",\"content\":\"ok\"}".to_string())
    }
}

#[async_trait]
impl BrowserAdapter for NoopBrowser {
    async fn launch(&self) -> Result<()> {
        Ok(())
    }

    async fn login_state(&self) -> Result<LoginState> {
        Ok(self.login.lock().unwrap().unwrap_or(LoginState::LoggedIn))
    }

    async fn wait_for_login(&self, _wait: Option<std::time::Duration>) -> Result<LoginState> {
        self.login_state().await
    }

    async fn current_url(&self) -> Result<String> {
        Ok("https://example.invalid/a/chat".into())
    }

    async fn page_title(&self) -> Result<String> {
        Ok("mock".into())
    }

    async fn user_agent(&self) -> Result<String> {
        Ok("mock-agent".into())
    }

    async fn screenshot(&self, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("mock.png");
        std::fs::write(&path, b"png")?;
        Ok(path)
    }

    async fn new_conversation(&self) -> Result<()> {
        self.assistant_count.store(0, Ordering::SeqCst);
        *self.last_reply.lock().unwrap() = None;
        self.conversations.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn apply_toggles(&self, _search: bool, _think: bool) -> Result<()> {
        Ok(())
    }

    async fn type_message(&self, text: &str) -> Result<()> {
        *self.buffer.lock().unwrap() = text.to_string();
        Ok(())
    }

    async fn clear_input(&self) -> Result<()> {
        self.buffer.lock().unwrap().clear();
        Ok(())
    }

    async fn input_value(&self) -> Result<String> {
        Ok(self.buffer.lock().unwrap().clone())
    }

    async fn submit(&self) -> Result<()> {
        let text = self.buffer.lock().unwrap().clone();
        if text.trim().is_empty() {
            return Err(AppError::SendFailed("空消息".into()));
        }
        self.submitted.lock().unwrap().push(text);
        if !self.keep_input_after_submit.load(Ordering::SeqCst) {
            self.buffer.lock().unwrap().clear();
        }
        let reply = self.next_reply();
        *self.last_reply.lock().unwrap() = Some(reply);
        self.assistant_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn is_generating(&self) -> Result<bool> {
        Ok(false)
    }

    async fn assistant_message_count(&self) -> Result<usize> {
        Ok(self.assistant_count.load(Ordering::SeqCst))
    }

    async fn last_assistant_text(&self) -> Result<Option<String>> {
        Ok(self.last_reply.lock().unwrap().clone())
    }

    async fn last_assistant_full_text(&self) -> Result<Option<String>> {
        self.last_assistant_text().await
    }

    async fn eval_json(&self, _js: &str) -> Result<Value> {
        Ok(json!({}))
    }

    async fn describe_page(&self) -> String {
        "url=https://example.invalid/a/chat title=mock".into()
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

// ------------------------------------------------------------------ 假对话后端

/// 可编排的 `ChatBackend`:按队列吐出回复,并记录每一次"带强提示词的发送"。
///
/// `injections` 是"同一对话内强提示词只注入一次"这条约束的断言点:
/// 一轮普通工具调用结束后,它不应该增长。
pub struct MockChatBackend {
    replies: Mutex<VecDeque<String>>,
    pub sent: Mutex<Vec<String>>,
    pub injections: Mutex<Vec<String>>,
    pub context_chars: AtomicUsize,
    pub rounds: AtomicUsize,
    pub thinking_chars: AtomicUsize,
    record: Mutex<Option<SessionRecord>>,
    seq: AtomicUsize,
}

impl MockChatBackend {
    pub fn new(replies: Vec<&str>) -> Self {
        Self::with_strings(replies.into_iter().map(str::to_string).collect())
    }

    pub fn with_strings(replies: Vec<String>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            sent: Mutex::new(Vec::new()),
            injections: Mutex::new(Vec::new()),
            context_chars: AtomicUsize::new(0),
            rounds: AtomicUsize::new(0),
            thinking_chars: AtomicUsize::new(0),
            record: Mutex::new(None),
            seq: AtomicUsize::new(0),
        }
    }

    pub fn set_replies(&self, replies: Vec<&str>) {
        *self.replies.lock().unwrap() = replies.into_iter().map(str::to_string).collect();
    }

    pub fn set_context_chars(&self, chars: usize) {
        self.context_chars.store(chars, Ordering::SeqCst);
    }

    pub fn set_thinking_chars(&self, chars: usize) {
        self.thinking_chars.store(chars, Ordering::SeqCst);
    }

    pub fn sent(&self) -> Vec<String> {
        self.sent.lock().unwrap().clone()
    }

    pub fn injections(&self) -> Vec<String> {
        self.injections.lock().unwrap().clone()
    }

    fn take_reply(&self) -> Reply {
        let text = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| "{\"type\":\"final\",\"content\":\"ok\"}".to_string());
        make_reply(&text, self.thinking_chars.load(Ordering::SeqCst))
    }

    fn new_conversation_id(&self) -> String {
        format!("cvt-{}", self.seq.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn record(&self, session_id: &str, conversation_id: &str, model: &str) -> SessionRecord {
        SessionRecord {
            session_id: session_id.to_string(),
            conversation_id: conversation_id.to_string(),
            conversation_count: 1,
            rounds_in_conversation: self.rounds.load(Ordering::SeqCst),
            prompt_injected: true,
            model_id: model.to_string(),
            context_chars: self.context_chars.load(Ordering::SeqCst),
            created_at: chrono::Local::now().to_rfc3339(),
            last_active: chrono::Local::now().to_rfc3339(),
            conversation_ids: vec![conversation_id.to_string()],
            last_summary: None,
        }
    }
}

#[async_trait]
impl ChatBackend for MockChatBackend {
    async fn start(
        &self,
        session_id: &str,
        first_message: &str,
        model: &ModelVariant,
    ) -> Result<(String, Reply)> {
        let conversation_id = self.new_conversation_id();
        self.injections.lock().unwrap().push(first_message.to_string());
        self.sent.lock().unwrap().push(first_message.to_string());
        self.rounds.store(1, Ordering::SeqCst);
        *self.record.lock().unwrap() =
            Some(self.record(session_id, &conversation_id, &model.id));
        Ok((conversation_id, self.take_reply()))
    }

    async fn send(&self, session_id: &str, payload: &str, _model: &ModelVariant) -> Result<Reply> {
        if self.record.lock().unwrap().is_none() {
            return Err(AppError::SessionNotFound(session_id.to_string()));
        }
        self.sent.lock().unwrap().push(payload.to_string());
        self.rounds.fetch_add(1, Ordering::SeqCst);
        let mut guard = self.record.lock().unwrap();
        let conversation_id = guard
            .as_ref()
            .map(|r| r.conversation_id.clone())
            .unwrap_or_default();
        let model_id = guard.as_ref().map(|r| r.model_id.clone()).unwrap_or_default();
        *guard = Some(self.record(session_id, &conversation_id, &model_id));
        drop(guard);
        Ok(self.take_reply())
    }

    async fn rotate(
        &self,
        session_id: &str,
        first_message: &str,
        model: &ModelVariant,
    ) -> Result<(String, Reply)> {
        let conversation_id = self.new_conversation_id();
        self.injections.lock().unwrap().push(first_message.to_string());
        self.sent.lock().unwrap().push(first_message.to_string());
        self.rounds.store(1, Ordering::SeqCst);
        let mut guard = self.record.lock().unwrap();
        let previous = guard.as_ref().cloned();
        let mut record = self.record(session_id, &conversation_id, &model.id);
        if let Some(previous) = previous {
            record.conversation_count = previous.conversation_count + 1;
            let mut ids = previous.conversation_ids;
            ids.push(conversation_id.clone());
            record.conversation_ids = ids;
        }
        *guard = Some(record);
        drop(guard);
        Ok((conversation_id, self.take_reply()))
    }

    async fn remember_prompt(&self, _session_id: &str, _prompt_block: &str) -> Result<()> {
        Ok(())
    }

    async fn conversation_id(&self, session_id: &str) -> Result<String> {
        self.record
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| r.conversation_id.clone())
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))
    }

    async fn context_chars(&self, _session_id: &str) -> Result<usize> {
        Ok(self.context_chars.load(Ordering::SeqCst))
    }

    async fn state(&self, session_id: &str) -> Result<SessionRecord> {
        self.record
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))
    }

    async fn list_sessions(&self) -> Result<Vec<SessionRecord>> {
        Ok(self
            .record
            .lock()
            .unwrap()
            .clone()
            .into_iter()
            .collect::<Vec<_>>())
    }

    async fn close_session(&self, session_id: &str) -> Result<()> {
        let mut guard = self.record.lock().unwrap();
        match guard.take() {
            Some(_) => Ok(()),
            None => Err(AppError::SessionNotFound(session_id.to_string())),
        }
    }
}

// ------------------------------------------------------------------ 本地 mock 页面

/// 一个尽量贴近目标页结构的最小页面:输入框、发送/停止按钮、新对话、头像、消息节点。
///
/// 它只用来验证"输入 → 发送 → 等待流式结束 → 提取文本"这条链路本身,
/// 不依赖 DeepSeek,也不会去猜它的私有接口。
pub const MOCK_PAGE_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
<head><meta charset="utf-8"><title>mock-chat</title></head>
<body>
  <div id="avatar" class="user-avatar">guest</div>
  <button id="new" class="new-chat">新对话</button>
  <div id="thread"></div>
  <textarea id="chat-input" placeholder="给 DeepSeek 发送消息"></textarea>
  <button id="send" class="send-button">发送</button>
  <button id="stop" class="stop-generating" hidden>停止生成</button>
<script>
  const thread = document.getElementById('thread');
  const input = document.getElementById('chat-input');
  const stop = document.getElementById('stop');
  let counter = 0;

  function typeOut(node, text) {
    stop.hidden = false;
    let i = 0;
    const timer = setInterval(() => {
      node.textContent = text.slice(0, ++i);
      if (i >= text.length) {
        clearInterval(timer);
        stop.hidden = true;
      }
    }, 10);
  }

  document.getElementById('send').addEventListener('click', () => {
    const text = input.value.trim();
    if (!text) { return; }
    const user = document.createElement('div');
    user.className = 'user message';
    user.textContent = text;
    thread.appendChild(user);

    const reply = document.createElement('div');
    reply.className = 'assistant markdown';
    reply.dataset.count = String(++counter);
    thread.appendChild(reply);

    input.value = '';
    typeOut(reply, '收到了 ' + text.length + ' 个字符');
  });

  document.getElementById('new').addEventListener('click', () => {
    thread.innerHTML = '';
    counter = 0;
  });
</script>
</body>
</html>
"#;

/// mock 页面使用的选择器,直接喂给 `SelectorConfig`。
pub fn mock_selectors() -> crate::browser::SelectorConfig {
    let mut selectors = crate::browser::SelectorConfig::default();
    selectors.login_input = vec!["#chat-input".into()];
    selectors.login_indicator = vec!["#avatar".into()];
    selectors.logged_in_url_markers = vec!["/mock".into()];
    selectors.new_chat_button = vec!["#new".into()];
    selectors.chat_input = vec!["#chat-input".into()];
    selectors.send_button = vec!["#send".into()];
    selectors.stop_button = vec!["#stop".into()];
    selectors.assistant_message = vec![".assistant".into()];
    selectors.user_message = vec![".user".into()];
    selectors.submit_with = crate::browser::SubmitMode::Button;
    selectors.type_strategy = crate::browser::TypeStrategy::TypeStrVerify;
    selectors
}
