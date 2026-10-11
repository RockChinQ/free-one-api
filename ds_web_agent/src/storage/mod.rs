//! 存储层:SQLite 记录会话/轮次/工具调用/错误 + JSONL 追加式事件流。
//!
//! 为什么两者都要:SQLite 负责"可查询的历史 + 累计用量",JSONL 负责"崩溃时也不丢的原始事件"
//! (写入 SQLite 需要事务,进程被杀时最后几条可能没落盘)。
//!
//! 所有写入都是尽力而为:存储失败只记 warn,绝不让记录问题打断正在进行的对话。

pub mod logger;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{AppConfig, PricingConfig, TokenEstimateMode};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundRecord {
    pub session_id: String,
    pub conversation_id: String,
    pub round: usize,
    /// `protocol`(首条注入) / `tool_result` / `correction`(解析失败重试,载荷里会带上完整强提示词)
    pub kind: String,
    pub payload_chars: usize,
    pub reply_chars: usize,
    /// 回复里的思考过程字符数(仅用于统计"网页对话实际消耗")
    pub thinking_chars: usize,
    pub model_json: Option<String>,
    pub parsed_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub session_id: String,
    pub conversation_id: String,
    pub round: usize,
    pub tool: String,
    pub ok: bool,
    pub duration_ms: u64,
    pub args: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// 某一个模型变体的用量。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub requests: u64,
    pub conversations: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayUsage {
    pub date: String,
    pub requests: u64,
    pub conversations: u64,
    pub total_tokens: u64,
    pub effective_tokens: u64,
    pub saved_amount: f64,
}

/// 使用量统计快照(§12)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    /// 历史创建的新对话个数
    pub conversations_created: u64,
    pub requests: u64,
    pub rounds: u64,
    pub tool_calls: u64,
    pub rotations: u64,
    pub errors: u64,

    /// 总 token:强提示词 + 返回 JSON + 模型思考 + 工具调用回填,全部按网页对话实际内容估算
    pub total_chars: u64,
    pub total_tokens: u64,
    pub total_prompt_tokens: u64,
    pub total_reply_tokens: u64,
    pub total_thinking_tokens: u64,
    pub total_tool_result_tokens: u64,

    /// 有效 token:只算 API 返回给客户端的内容 + 调用 API 发送的内容
    pub effective_input_chars: u64,
    pub effective_input_tokens: u64,
    pub effective_output_chars: u64,
    pub effective_output_tokens: u64,

    /// 累计节省金额:有效输入/输出按 API 价目折算
    pub saved_input_amount: f64,
    pub saved_output_amount: f64,
    pub saved_total_amount: f64,
    pub currency: String,
    /// 当前 token 估算口径,便于解读上面的数字
    pub token_estimate: String,
    pub by_model: Vec<ModelUsage>,
    pub daily: Vec<DayUsage>,
}

/// 累计计数器键名;集中在这里避免各处写字符串拼错。
mod counters {
    pub const CONVERSATIONS: &str = "conversations_created";
    pub const REQUESTS: &str = "requests";
    pub const ROUNDS: &str = "rounds";
    pub const TOOL_CALLS: &str = "tool_calls";
    pub const ROTATIONS: &str = "rotations";
    pub const ERRORS: &str = "errors";
    pub const PROMPT_CHARS: &str = "chars_prompt";
    pub const REPLY_CHARS: &str = "chars_reply";
    pub const THINKING_CHARS: &str = "chars_thinking";
    pub const TOOL_RESULT_CHARS: &str = "chars_tool_result";
    pub const EFFECTIVE_IN_CHARS: &str = "chars_effective_in";
    pub const EFFECTIVE_OUT_CHARS: &str = "chars_effective_out";
}

pub struct Store {
    conn: Mutex<Connection>,
    jsonl_dir: Option<PathBuf>,
    /// 是否把 HTTP 请求/响应正文写进记录(预留,当前由 record_http_bodies 控制)
    #[allow(dead_code)]
    record_bodies: bool,
    pub enabled: bool,
    mode: TokenEstimateMode,
    pricing: PricingConfig,
    text_limit: usize,
}

impl Store {
    pub fn open(cfg: &AppConfig) -> Result<Self> {
        if !cfg.storage.enabled {
            tracing::warn!("[storage].enabled = false,记录将只存在于进程内存中");
            return Self::memory_with(cfg);
        }
        if let Some(parent) = std::path::Path::new(&cfg.storage.sqlite_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Storage(format!("创建 {} 失败: {e}", parent.display())))?;
        }
        let conn = Connection::open(&cfg.storage.sqlite_path)
            .map_err(|e| AppError::Storage(format!("打开 SQLite 失败: {e}")))?;
        let dir = PathBuf::from(&cfg.storage.jsonl_dir);
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::Storage(format!("创建 JSONL 目录失败: {e}")))?;
        Self::from_conn(conn, cfg, Some(dir))
    }

    /// 单元测试用:不开文件。
    pub fn memory() -> Result<Self> {
        Self::memory_with(&AppConfig::default())
    }

    fn memory_with(cfg: &AppConfig) -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AppError::Storage(format!("创建内存 SQLite 失败: {e}")))?;
        Self::from_conn(conn, cfg, None)
    }

    fn from_conn(conn: Connection, cfg: &AppConfig, jsonl_dir: Option<PathBuf>) -> Result<Self> {
        let store = Self {
            conn: Mutex::new(conn),
            jsonl_dir,
            record_bodies: cfg.storage.record_http_bodies,
            enabled: cfg.storage.enabled,
            mode: cfg.context.mode,
            pricing: cfg.pricing.clone(),
            text_limit: cfg.agent.record_text_limit.max(200),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.lock()?;
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS sessions (
  session_id TEXT PRIMARY KEY,
  model TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  conversations INTEGER NOT NULL DEFAULT 0,
  rotations INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS conversations (
  conversation_id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  model TEXT NOT NULL,
  label TEXT,
  created_at TEXT NOT NULL,
  closed_reason TEXT,
  rounds INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS rounds (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  conversation_id TEXT NOT NULL,
  round INTEGER NOT NULL,
  kind TEXT NOT NULL,
  payload_chars INTEGER NOT NULL,
  reply_chars INTEGER NOT NULL,
  thinking_chars INTEGER NOT NULL,
  model_json TEXT,
  parsed_type TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tool_calls (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  conversation_id TEXT NOT NULL,
  round INTEGER NOT NULL,
  tool TEXT NOT NULL,
  ok INTEGER NOT NULL,
  duration_ms INTEGER NOT NULL,
  args TEXT,
  result TEXT,
  error TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS errors (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT,
  code TEXT NOT NULL,
  message TEXT NOT NULL,
  context TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS counters (
  key TEXT PRIMARY KEY,
  value INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_rounds_session ON rounds(session_id, id);
CREATE INDEX IF NOT EXISTS idx_tool_calls_session ON tool_calls(session_id, id);
"#,
        )
        .map_err(|e| AppError::Storage(format!("初始化表结构失败: {e}")))?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| AppError::Storage(format!("SQLite 连接锁被污染: {e}")))
    }

    fn truncate(&self, text: &str) -> String {
        if text.chars().count() <= self.text_limit {
            return text.to_string();
        }
        let head: String = text.chars().take(self.text_limit).collect();
        format!("{head}…[截断,原文 {} 字符]", text.chars().count())
    }

    fn now() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    /// 追加一行 JSONL;失败只 warn。
    pub fn append_event(&self, kind: &str, payload: &Value) {
        let Some(dir) = &self.jsonl_dir else {
            return;
        };
        let line = serde_json::json!({
            "ts": Self::now(),
            "kind": kind,
            "payload": payload,
        });
        let path = dir.join(format!("{}.jsonl", chrono::Utc::now().format("%Y-%m-%d")));
        let res = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(f, "{}", serde_json::to_string(&line).unwrap_or_default())
            });
        if let Err(e) = res {
            tracing::warn!(error = %e, path = %path.display(), "写入 JSONL 失败");
        }
    }

    fn bump(&self, key: &str, delta: i64) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO counters(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = value + ?2",
            params![key, delta],
        )
        .map_err(|e| AppError::Storage(format!("更新计数器 {key} 失败: {e}")))?;
        Ok(())
    }

    pub fn record_session_created(&self, session_id: &str, model: &str) {
        let now = Self::now();
        let res = (|| -> Result<()> {
            let conn = self.lock()?;
            conn.execute(
                "INSERT INTO sessions(session_id, model, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?3)
                 ON CONFLICT(session_id) DO UPDATE SET model = ?2, updated_at = ?3",
                params![session_id, model, now],
            )
            .map_err(|e| AppError::Storage(format!("写入 session 失败: {e}")))?;
            Ok(())
        })();
        self.report(res, "record_session_created");
    }

    /// 记录一次"新建对话"。§12 的"历史创建新对话个数"就是这张表的行数。
    pub fn record_conversation_created(
        &self,
        conversation_id: &str,
        session_id: &str,
        model: &str,
        label: &str,
    ) {
        let now = Self::now();
        let res = (|| -> Result<()> {
            {
                let conn = self.lock()?;
                conn.execute(
                    "INSERT OR REPLACE INTO conversations(conversation_id, session_id, model, label, created_at, rounds)
                     VALUES(?1, ?2, ?3, ?4, ?5, 0)",
                    params![conversation_id, session_id, model, label, now],
                )
                .map_err(|e| AppError::Storage(format!("写入 conversation 失败: {e}")))?;
                conn.execute(
                    "UPDATE sessions SET updated_at = ?2, conversations = conversations + 1 WHERE session_id = ?1",
                    params![session_id, now],
                )
                .ok();
            }
            self.bump(counters::CONVERSATIONS, 1)
        })();
        self.report(res, "record_conversation_created");
    }

    pub fn record_rotation(&self, session_id: &str) {
        let res = (|| -> Result<()> {
            {
                let conn = self.lock()?;
                conn.execute(
                    "UPDATE sessions SET rotations = rotations + 1, updated_at = ?2 WHERE session_id = ?1",
                    params![session_id, Self::now()],
                )
                .ok();
            }
            self.bump(counters::ROTATIONS, 1)
        })();
        self.report(res, "record_rotation");
    }

    pub fn record_round(&self, record: &RoundRecord) {
        let res = (|| -> Result<()> {
            {
                let conn = self.lock()?;
                conn.execute(
                    "INSERT INTO rounds(session_id, conversation_id, round, kind, payload_chars, reply_chars, thinking_chars, model_json, parsed_type, created_at)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                    params![
                        record.session_id,
                        record.conversation_id,
                        record.round as i64,
                        record.kind,
                        record.payload_chars as i64,
                        record.reply_chars as i64,
                        record.thinking_chars as i64,
                        record.model_json.as_deref().map(|s| self.truncate(s)),
                        record.parsed_type,
                        Self::now(),
                    ],
                )
                .map_err(|e| AppError::Storage(format!("写入 round 失败: {e}")))?;
                conn.execute(
                    "UPDATE conversations SET rounds = rounds + 1 WHERE conversation_id = ?1",
                    params![record.conversation_id],
                )
                .ok();
            }
            self.bump(counters::ROUNDS, 1)?;
            // 总 token 的四个来源:注入的提示词/协议载荷、模型返回、模型思考、工具调用回填
            self.bump(counters::PROMPT_CHARS, record.payload_chars as i64)?;
            self.bump(counters::REPLY_CHARS, record.reply_chars as i64)?;
            self.bump(counters::THINKING_CHARS, record.thinking_chars as i64)
        })();
        self.report(res, "record_round");
    }

    /// 工具结果的回填也算总 token 的一部分。
    pub fn record_tool_result_chars(&self, chars: usize) {
        let res = self.bump(counters::TOOL_RESULT_CHARS, chars as i64);
        self.report(res, "record_tool_result_chars");
    }

    pub fn record_tool_call(&self, record: &ToolCallRecord) {
        let res = (|| -> Result<()> {
            {
                let conn = self.lock()?;
                conn.execute(
                    "INSERT INTO tool_calls(session_id, conversation_id, round, tool, ok, duration_ms, args, result, error, created_at)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                    params![
                        record.session_id,
                        record.conversation_id,
                        record.round as i64,
                        record.tool,
                        record.ok as i64,
                        record.duration_ms as i64,
                        record.args.as_deref().map(|s| self.truncate(s)),
                        record.result.as_deref().map(|s| self.truncate(s)),
                        record.error.as_deref().map(|s| self.truncate(s)),
                        Self::now(),
                    ],
                )
                .map_err(|e| AppError::Storage(format!("写入 tool_call 失败: {e}")))?;
            }
            self.bump(counters::TOOL_CALLS, 1)
        })();
        self.report(res, "record_tool_call");
    }

    pub fn record_error(&self, session_id: Option<&str>, code: &str, message: &str, context: &str) {
        let res = (|| -> Result<()> {
            {
                let conn = self.lock()?;
                conn.execute(
                    "INSERT INTO errors(session_id, code, message, context, created_at) VALUES(?1,?2,?3,?4,?5)",
                    params![session_id, code, message, self.truncate(context), Self::now()],
                )
                .map_err(|e| AppError::Storage(format!("写入 error 失败: {e}")))?;
            }
            self.bump(counters::ERRORS, 1)
        })();
        self.report(res, "record_error");
    }

    /// 有效 token:API 发送给模型的内容(用户输入 + 工具结果回填)与 API 返回给客户端的内容。
    pub fn record_effective(&self, input_chars: usize, output_chars: usize) {
        let res = (|| -> Result<()> {
            self.bump(counters::EFFECTIVE_IN_CHARS, input_chars as i64)?;
            self.bump(counters::EFFECTIVE_OUT_CHARS, output_chars as i64)
        })();
        self.report(res, "record_effective");
    }

    /// API 侧发起一次请求。
    pub fn record_request(&self, session_id: &str, model: &str, is_new_session: bool) {
        let res = (|| -> Result<()> {
            {
                let conn = self.lock()?;
                conn.execute(
                    "UPDATE sessions SET updated_at = ?2 WHERE session_id = ?1",
                    params![session_id, Self::now()],
                )
                .ok();
            }
            self.bump(counters::REQUESTS, 1)?;
            if is_new_session {
                self.record_session_created(session_id, model);
            }
            Ok(())
        })();
        self.report(res, "record_request");
    }

    fn report(&self, res: Result<()>, what: &str) {
        if let Err(e) = res {
            tracing::warn!(error = %e, op = what, "存储写入失败(不影响对话继续)");
            if let Ok(conn) = self.conn.lock() {
                let _ = conn.execute(
                    "INSERT INTO errors(session_id, code, message, context, created_at) VALUES(NULL, 'storage', ?1, ?2, ?3)",
                    params![e.to_string(), what, Self::now()],
                );
            }
        }
    }

    fn counter(&self, key: &str) -> u64 {
        let Ok(conn) = self.conn.lock() else {
            return 0;
        };
        conn.query_row(
            "SELECT value FROM counters WHERE key = ?1",
            params![key],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .ok()
        .flatten()
        .unwrap_or(0)
        .max(0) as u64
    }

    fn tokens(&self, chars: u64) -> u64 {
        // 与 ContextManager 用同一口径,避免两处数字对不上
        if chars == 0 {
            return 0;
        }
        match self.mode {
            TokenEstimateMode::CharsDiv4 => chars.div_ceil(4),
            TokenEstimateMode::CjkAware => {
                // 累计计数只有字符总数,无法再分 CJK;按 chars/4 近似并向上取整
                chars.div_ceil(4)
            }
        }
    }

    fn money(&self, tokens: u64, per_million: f64) -> f64 {
        (tokens as f64 / 1_000_000.0) * per_million
    }

    pub fn usage(&self) -> Result<UsageSnapshot> {
        let prompt_chars = self.counter(counters::PROMPT_CHARS);
        let reply_chars = self.counter(counters::REPLY_CHARS);
        let thinking_chars = self.counter(counters::THINKING_CHARS);
        let tool_chars = self.counter(counters::TOOL_RESULT_CHARS);
        let effective_in_chars = self.counter(counters::EFFECTIVE_IN_CHARS);
        let effective_out_chars = self.counter(counters::EFFECTIVE_OUT_CHARS);

        let total_chars = prompt_chars + reply_chars + thinking_chars + tool_chars;
        let total_tokens = self.tokens(total_chars);
        let effective_input_tokens = self.tokens(effective_in_chars);
        let effective_output_tokens = self.tokens(effective_out_chars);

        let saved_input_amount = self.money(effective_input_tokens, self.pricing.input_per_million);
        let saved_output_amount =
            self.money(effective_output_tokens, self.pricing.output_per_million);

        Ok(UsageSnapshot {
            conversations_created: self.counter(counters::CONVERSATIONS),
            requests: self.counter(counters::REQUESTS),
            rounds: self.counter(counters::ROUNDS),
            tool_calls: self.counter(counters::TOOL_CALLS),
            rotations: self.counter(counters::ROTATIONS),
            errors: self.counter(counters::ERRORS),
            total_chars,
            total_tokens,
            total_prompt_tokens: self.tokens(prompt_chars),
            total_reply_tokens: self.tokens(reply_chars),
            total_thinking_tokens: self.tokens(thinking_chars),
            total_tool_result_tokens: self.tokens(tool_chars),
            effective_input_chars: effective_in_chars,
            effective_input_tokens,
            effective_output_chars: effective_out_chars,
            effective_output_tokens,
            saved_input_amount,
            saved_output_amount,
            saved_total_amount: saved_input_amount + saved_output_amount,
            currency: self.pricing.currency.clone(),
            token_estimate: match self.mode {
                TokenEstimateMode::CharsDiv4 => "chars_div_4".into(),
                TokenEstimateMode::CjkAware => "cjk_aware(累计值按 chars/4 近似)".into(),
            },
            by_model: self.by_model().unwrap_or_default(),
            daily: self.daily(14).unwrap_or_default(),
        })
    }

    fn by_model(&self) -> Result<Vec<ModelUsage>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT s.model,
                        COUNT(DISTINCT c.conversation_id) AS conversations,
                        COALESCE(SUM(r.payload_chars + r.reply_chars + r.thinking_chars), 0) AS chars
                 FROM sessions s
                 LEFT JOIN conversations c ON c.session_id = s.session_id
                 LEFT JOIN rounds r ON r.session_id = s.session_id
                 GROUP BY s.model
                 ORDER BY chars DESC",
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let model: String = row.get(0)?;
                let conversations: i64 = row.get(1)?;
                let chars: i64 = row.get(2)?;
                Ok(ModelUsage {
                    model,
                    requests: 0,
                    conversations: conversations.max(0) as u64,
                    total_tokens: chars.max(0) as u64 / 4,
                })
            })
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Storage(e.to_string()))?);
        }
        Ok(out)
    }

    /// 最近 `days` 天的用量(按本地日期聚合)。
    pub fn daily(&self, days: u32) -> Result<Vec<DayUsage>> {
        let since = (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();
        let rows = {
            let conn = self.lock()?;
            let mut stmt = conn
                .prepare(
                    "SELECT substr(created_at, 1, 10) AS day,
                            COUNT(*) AS rounds,
                            COALESCE(SUM(payload_chars + reply_chars + thinking_chars), 0) AS chars
                     FROM rounds WHERE created_at >= ?1
                     GROUP BY day ORDER BY day",
                )
                .map_err(|e| AppError::Storage(e.to_string()))?;
            let mapped = stmt
                .query_map(params![since], |row| {
                    let date: String = row.get(0)?;
                    let rounds: i64 = row.get(1)?;
                    let chars: i64 = row.get(2)?;
                    Ok((date, rounds.max(0) as u64, chars.max(0) as u64))
                })
                .map_err(|e| AppError::Storage(e.to_string()))?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| AppError::Storage(e.to_string()))?;
            mapped
        };

        Ok(rows
            .into_iter()
            .map(|(date, rounds, chars)| {
                let tokens = chars.div_ceil(4);
                DayUsage {
                    date,
                    requests: rounds,
                    conversations: 0,
                    total_tokens: tokens,
                    effective_tokens: 0,
                    saved_amount: self.money(tokens, self.pricing.input_per_million),
                }
            })
            .collect())
    }

    /// 会话维度的历史(便于排查"哪次请求用了多少")。
    pub fn recent_sessions(&self, limit: usize) -> Result<Vec<Value>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT s.session_id, s.model, s.created_at, s.conversations, s.rotations,
                        (SELECT COUNT(*) FROM rounds r WHERE r.session_id = s.session_id)
                 FROM sessions s ORDER BY s.updated_at DESC LIMIT ?1",
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(serde_json::json!({
                    "session_id": row.get::<_, String>(0)?,
                    "model": row.get::<_, String>(1)?,
                    "created_at": row.get::<_, String>(2)?,
                    "conversations": row.get::<_, i64>(3)?,
                    "rotations": row.get::<_, i64>(4)?,
                    "rounds": row.get::<_, i64>(5)?,
                }))
            })
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Storage(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn recent_errors(&self, limit: usize) -> Result<Vec<Value>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT created_at, code, message, session_id, context
                 FROM errors ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(serde_json::json!({
                    "created_at": row.get::<_, String>(0)?,
                    "code": row.get::<_, String>(1)?,
                    "message": row.get::<_, String>(2)?,
                    "session_id": row.get::<_, Option<String>>(3)?,
                    "context": row.get::<_, Option<String>>(4)?,
                }))
            })
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Storage(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn tool_call_stats(&self) -> Result<Vec<Value>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT tool, COUNT(*), SUM(CASE WHEN ok = 1 THEN 1 ELSE 0 END), AVG(duration_ms)
                 FROM tool_calls GROUP BY tool ORDER BY COUNT(*) DESC",
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "tool": row.get::<_, String>(0)?,
                    "calls": row.get::<_, i64>(1)?,
                    "ok": row.get::<_, Option<i64>>(2)?,
                    "avg_ms": row.get::<_, Option<f64>>(3)?,
                }))
            })
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Storage(e.to_string()))?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(session: &str, conversation: &str, payload: usize, reply: usize) -> RoundRecord {
        RoundRecord {
            session_id: session.into(),
            conversation_id: conversation.into(),
            round: 1,
            kind: "protocol".into(),
            payload_chars: payload,
            reply_chars: reply,
            thinking_chars: 0,
            model_json: Some("{\"type\":\"final\"}".into()),
            parsed_type: Some("final".into()),
        }
    }

    #[test]
    fn counters_add_up_and_prices_are_applied() {
        let store = Store::memory().unwrap();
        store.record_session_created("s1", "ds-web-search");
        store.record_conversation_created("c1", "s1", "ds-web-search", "首轮");
        store.record_conversation_created("c2", "s1", "ds-web-search", "轮换");
        store.record_round(&round("s1", "c1", 4_000_000, 4));
        store.record_tool_result_chars(4000);
        store.record_effective(4_000_000, 8000);

        let usage = store.usage().unwrap();
        assert_eq!(usage.conversations_created, 2, "应统计新建对话个数");
        assert_eq!(usage.rounds, 1);
        assert_eq!(usage.total_chars, 4_000_000 + 4 + 4000, "总 token 应含提示词/返回/工具回填");
        assert_eq!(usage.total_tokens, (4_000_000u64 + 4004).div_ceil(4));
        assert_eq!(usage.effective_input_tokens, 1_000_000);
        assert_eq!(usage.effective_output_tokens, 2000);
        // 100 万输入 token × 9 元 + 2000 输出 token × 27 元 / 百万
        assert!((usage.saved_input_amount - 9.0).abs() < 1e-9);
        assert!((usage.saved_output_amount - 0.054).abs() < 1e-9);
        assert!((usage.saved_total_amount - 9.054).abs() < 1e-9);
        assert_eq!(usage.currency, "CNY");
    }

    #[test]
    fn thinking_chars_count_toward_total_tokens() {
        let store = Store::memory().unwrap();
        let mut record = round("s1", "c1", 0, 0);
        record.thinking_chars = 400;
        store.record_round(&record);
        let usage = store.usage().unwrap();
        assert_eq!(usage.total_thinking_tokens, 100);
        assert_eq!(usage.total_tokens, 100);
        // 思考过程不计入"有效 token"
        assert_eq!(usage.effective_output_tokens, 0);
    }

    #[test]
    fn tool_calls_and_errors_are_persisted() {
        let store = Store::memory().unwrap();
        store.record_tool_call(&ToolCallRecord {
            session_id: "s1".into(),
            conversation_id: "c1".into(),
            round: 1,
            tool: "get_time".into(),
            ok: true,
            duration_ms: 3,
            args: Some("{}".into()),
            result: Some("{\"datetime\":\"now\"}".into()),
            error: None,
        });
        store.record_error(Some("s1"), "selector_not_found", "找不到输入框", "url=...");
        let usage = store.usage().unwrap();
        assert_eq!(usage.tool_calls, 1);
        assert_eq!(usage.errors, 1);
        assert_eq!(store.tool_call_stats().unwrap().len(), 1);
        assert_eq!(store.recent_errors(10).unwrap()[0]["code"], "selector_not_found");
    }

    #[test]
    fn long_text_is_truncated_before_persisting() {
        let store = Store::memory().unwrap();
        let long = "x".repeat(50_000);
        let mut record = round("s1", "c1", 1, 1);
        record.model_json = Some(long);
        store.record_round(&record); // 不应 panic,也不应写入 5 万字符
        assert_eq!(store.usage().unwrap().rounds, 1);
    }

    #[test]
    fn recent_sessions_reports_aggregates() {
        let store = Store::memory().unwrap();
        store.record_session_created("s1", "ds-web-think");
        store.record_conversation_created("c1", "s1", "ds-web-think", "首轮");
        store.record_rotation("s1");
        let sessions = store.recent_sessions(5).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["rotations"], 1);
        assert_eq!(sessions[0]["conversations"], 1);
    }
}
