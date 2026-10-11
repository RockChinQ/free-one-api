//! 工具层:注册表 + 内置示例工具。
//!
//! 安全边界集中在 [`ToolRegistry::execute`]:白名单、危险工具开关、参数校验、执行超时
//! 都在这一处强制,内置工具自身不需要重复这些检查。

pub mod builtin;
pub mod registry;

pub use registry::{Tool, ToolRegistry, ToolSource, ToolSpec};
