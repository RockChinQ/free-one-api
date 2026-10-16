//! 对话层:会话状态 + 「强提示词只注入一次」的约束 + 上下文轮换。
//!
//! 关键抽象是 [`ChatBackend`]:AgentLoop 只依赖它,于是"强提示词只出现一次""工具结果回填"
//! 这类约束可以在**不启动浏览器**的情况下用单元测试锁死(见 tests/agent_loop_test.rs)。

pub mod context_manager;
pub mod conversation_adapter;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::ModelVariant;
use crate::error::Result;

pub use context_manager::{ContextDecision, ContextManager, RotateReason};
pub use conversation_adapter::BrowserChatBackend;

/// 会话内的持久状态。放在后端里(而不是 API 层),这样断线重连仍然能继续同一对话。
#[derive(Debug, Clone)]
pub struct SessionState {
    pub session_id: String,
    /// 当前所在对话(每次轮换换新的)
    pub conversation_id: String,
    /// 本会话创建过的所有对话,用于 §12 的"历史创建新对话个数"
    pub conversation_ids: Vec<String>,
    /// 当前对话是否已经注入过强提示词——同一对话内只允许一次
    pub prompt_injected: bool,
    /// 当前对话累计的估算字符数(发送 + 接收)
    pub context_chars: usize,
    /// 当前对话内已经发生的轮次
    pub rounds_in_conversation: usize,
    pub model_id: String,
    pub created_at: chrono::DateTime<chrono::Local>,
    pub last_active: chrono::DateTime<chrono::Local>,
    /// 首次注入的强提示词原文:轮换到新对话时原样再注入一次
    pub prompt_block: Option<String>,
    /// 已发送的载荷(含强提示词/用户输入/工具结果),供轮换时生成摘要与记录
    pub sent: Vec<String>,
    /// 已收到的模型输出
    pub received: Vec<String>,
    /// 最近一次摘要(轮换时生成)
    pub last_summary: Option<String>,
}

impl SessionState {
    pub fn new(session_id: &str, conversation_id: &str, model_id: &str) -> Self {
        let now = chrono::Local::now();
        Self {
            session_id: session_id.to_string(),
            conversation_id: conversation_id.to_string(),
            conversation_ids: vec![conversation_id.to_string()],
            prompt_injected: false,
            context_chars: 0,
            rounds_in_conversation: 0,
            model_id: model_id.to_string(),
            created_at: now,
            last_active: now,
            prompt_block: None,
            sent: Vec::new(),
            received: Vec::new(),
            last_summary: None,
        }
    }

    pub fn touch(&mut self) {
        self.last_active = chrono::Local::now();
    }

    /// 记录一次发送。
    ///
    /// `is_prompt_injection` 只由真正的首条消息传 true——这是"同一对话内强提示词只注入一次"
    /// 这条约束的唯一写入点,便于测试断言。
    pub fn record_sent(&mut self, text: &str, is_prompt_injection: bool) {
        self.context_chars += text.chars().count();
        self.sent.push(text.to_string());
        if is_prompt_injection {
            self.prompt_injected = true;
        }
        self.touch();
    }

    pub fn record_received(&mut self, text: &str) {
        self.context_chars += text.chars().count();
        self.received.push(text.to_string());
        self.rounds_in_conversation += 1;
        self.touch();
    }

    /// 轮换到新对话:强提示词需要重新注入一次,所以 `prompt_injected` 归零。
    pub fn rotate_to(&mut self, conversation_id: &str) {
        self.conversation_id = conversation_id.to_string();
        self.conversation_ids.push(conversation_id.to_string());
        self.prompt_injected = false;
        self.context_chars = 0;
        self.rounds_in_conversation = 0;
        self.touch();
    }

    pub fn as_record(&self) -> SessionRecord {
        SessionRecord {
            session_id: self.session_id.clone(),
            conversation_id: self.conversation_id.clone(),
            conversation_count: self.conversation_ids.len(),
            rounds_in_conversation: self.rounds_in_conversation,
            prompt_injected: self.prompt_injected,
            model_id: self.model_id.clone(),
            context_chars: self.context_chars,
            created_at: self.created_at.to_rfc3339(),
            last_active: self.last_active.to_rfc3339(),
            conversation_ids: self.conversation_ids.clone(),
            last_summary: self.last_summary.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub conversation_id: String,
    /// 本会话创建过的对话个数
    pub conversation_count: usize,
    pub rounds_in_conversation: usize,
    pub prompt_injected: bool,
    pub model_id: String,
    pub context_chars: usize,
    pub created_at: String,
    pub last_active: String,
    pub conversation_ids: Vec<String>,
    pub last_summary: Option<String>,
}

/// 一轮回复的内容。
///
/// 同时带"正文"和"含思考过程的全文"两个字段:正文交给解析器,全文只用于 §12 的 token 统计,
/// 因为模型思考也属于网页对话实际消耗。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub text: String,
    pub full_text: String,
    pub chars: usize,
    pub full_chars: usize,
    pub assistant_messages: usize,
    pub elapsed_ms: u64,
    /// 文本连续稳定了多久才判定"生成结束"
    pub stable_ms: u64,
}

impl Reply {
    /// 思考过程字符数 = 全文 - 正文。允许为 0,不会回退成负数。
    pub fn thinking_chars(&self) -> usize {
        self.full_chars.saturating_sub(self.chars)
    }
}

/// 对话后端的抽象。
#[async_trait]
pub trait ChatBackend: Send + Sync {
    /// 新建对话并发出首条消息:**强提示词 + 首个载荷**。返回 conversation_id。
    ///
    /// 调用方负责把首条消息拼好;这里是 `prompt_injected` 的唯一写入点,
    /// [`ChatBackend::send`] 永远不能把它再置为 true。
    async fn start(
        &self,
        session_id: &str,
        first_message: &str,
        model: &ModelVariant,
    ) -> Result<(String, Reply)>;

    /// 普通轮次:只发内容,绝不重复注入强提示词。
    /// 内部完成"发送 → 等流式结束 → 提取正文",成功返回该轮回复。
    async fn send(
        &self,
        session_id: &str,
        payload: &str,
        model: &ModelVariant,
    ) -> Result<Reply>;

    /// 上下文超限时轮换,顺序不可调换:
    /// 1. 在**快满的这个对话**里请求摘要(摘要必须由该对话自己产出);
    /// 2. 点「新对话」;
    /// 3. 新对话首条消息 = 强提示词 + 摘要 + 当前任务状态;之后不再重复注入强提示词。
    ///
    /// 步骤 1 由调用方先用 [`ChatBackend::send`] 完成,这里只负责 2 和 3。
    async fn rotate(
        &self,
        session_id: &str,
        first_message: &str,
        model: &ModelVariant,
    ) -> Result<(String, Reply)>;

    /// 上传文件到当前对话。path 是本地文件路径。
    async fn upload_file(&self, session_id: &str, path: &str) -> Result<()>;

    /// 保存强提示词原文,轮换时需要重新注入一次。
    async fn remember_prompt(&self, session_id: &str, prompt_block: &str) -> Result<()>;

    async fn conversation_id(&self, session_id: &str) -> Result<String>;

    async fn context_chars(&self, session_id: &str) -> Result<usize>;

    async fn state(&self, session_id: &str) -> Result<SessionRecord>;

    async fn list_sessions(&self) -> Result<Vec<SessionRecord>>;

    async fn close_session(&self, session_id: &str) -> Result<()>;
}
