//! Agent 层:强提示词 → JSON 协议 → 工具调用 → 多轮循环 → 上下文轮换。

pub mod agent_loop;
pub mod json_parser;
pub mod prompt_builder;

pub use agent_loop::{
    AgentEvent, AgentLoop, AgentOutcome, AgentRun, ClientToolCall, ClientToolSpec, RotationRecord,
    RunUsage, StepKind, StepRecord,
};
pub use json_parser::{parse_action, ExtractStrategy, ModelAction, ParsedAction};
pub use prompt_builder::{describe_model_mode, PromptBuilder, PromptContext};
