//! DS_WebAgent:把 DeepSeek 网页对话封装成本地 HTTP API(OpenAI 兼容外观)。
//!
//! 分层:
//! - [`browser`]:真实浏览器控制,`BrowserAdapter` 是唯一需要替换的接口;
//! - [`conversation`]:把"一次收发"变成事务(发送生效校验、流式结束判定、上下文轮换);
//! - [`agent`]:强提示词 + JSON 协议 + 工具调用循环;
//! - [`tools`]:工具注册表与内置工具(白名单、超时、参数校验);
//! - [`api`]:axum 路由与请求/响应模型;
//! - [`storage`]:SQLite/JSONL 记录与使用量统计;
//! - [`testing`]:假浏览器 / 假对话后端 / mock 页面,供测试使用。

pub mod agent;
pub mod api;
pub mod browser;
pub mod config;
pub mod conversation;
pub mod error;
pub mod storage;
pub mod testing;
pub mod tools;

pub use config::AppConfig;
pub use error::{AppError, Result};

/// 版本号来自 Cargo.toml,便于 /health 上报。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
