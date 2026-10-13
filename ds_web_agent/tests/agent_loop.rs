//! AgentLoop 状态机集成测试:用 MockChatBackend 驱动完整的多轮流程,
//! 不依赖真实浏览器或 DeepSeek。
//!
//! 这些测试锁的是协议级不变量:
//! - 同一对话内强提示词只注入一次(解析失败重试时会整体重发);
//! - tool_call → 本地执行 → TOOL_RESULT 回填 → final;
//! - 解析失败发修正提示后能恢复;
//! - 上下文超限时轮换,且新对话重新注入一次强提示词;
//! - 超过 max_rounds 直接报错。

use std::sync::Arc;

use ds_webagent::agent::{AgentLoop, AgentRun};
use ds_webagent::config::{AppConfig, ModelVariant};
use ds_webagent::conversation::ChatBackend;
use ds_webagent::error::AppError;
use ds_webagent::storage::Store;
use ds_webagent::testing::MockChatBackend;
use ds_webagent::tools::ToolRegistry;

fn harness(replies: Vec<&str>) -> (Arc<AgentLoop>, Arc<MockChatBackend>) {
    let cfg = Arc::new(AppConfig::default());
    let store = Arc::new(Store::memory().expect("内存库"));
    let backend = Arc::new(MockChatBackend::new(replies));
    let backend_dyn: Arc<dyn ChatBackend> = backend.clone();
    let registry = Arc::new(ToolRegistry::new(&cfg));
    let agent = Arc::new(AgentLoop::new(backend_dyn, registry, cfg, store));
    (agent, backend)
}

fn default_model() -> ModelVariant {
    AppConfig::default()
        .models
        .default_variant()
        .expect("默认变体存在")
        .clone()
}

fn build_run(input: &str, tools: &[&str], max_rounds: usize) -> AgentRun {
    AgentRun {
        session_id: None,
        input: input.into(),
        tools: tools.iter().map(|s| s.to_string()).collect(),
        client_tools: Vec::new(),
        max_rounds,
        model: default_model(),
        events: None,
        cancel: None,
        file_paths: Vec::new(),
    }
}

const TOOL_CALL_TIME: &str = r#"{"type":"tool_call","tool":"get_time","arguments":{}}"#;
const FINAL_DONE: &str = r#"{"type":"final","content":"done"}"#;

#[tokio::test]
async fn tool_call_then_final() {
    let (agent, backend) = harness(vec![TOOL_CALL_TIME, FINAL_DONE]);
    let outcome = agent.run(build_run("现在几点", &["get_time"], 8)).await.unwrap();
    assert_eq!(outcome.content, "done");
    assert_eq!(outcome.steps.len(), 2);
    assert!(outcome.prompt_injected);
    assert!(backend.sent().iter().any(|m| m.contains("TOOL_RESULT")));
}

#[tokio::test]
async fn strong_prompt_injected_exactly_once_per_conversation() {
    let (agent, backend) = harness(vec![TOOL_CALL_TIME, FINAL_DONE]);
    let _ = agent.run(build_run("现在几点", &["get_time"], 8)).await.unwrap();
    let injections = backend.injections();
    assert_eq!(injections.len(), 1, "同一对话内强提示词应只注入一次");
    assert!(
        injections[0].contains("工具调用代理"),
        "首条消息必须包含强提示词"
    );
}

#[tokio::test]
async fn parse_failure_is_corrected_then_final() {
    let (agent, _backend) = harness(vec!["这不是 JSON", FINAL_DONE]);
    let outcome = agent.run(build_run("随便问", &[], 8)).await.unwrap();
    assert_eq!(outcome.content, "done");
    assert!(outcome.rounds >= 2);
}

/// 回归:解析失败的重试必须重新注入**完整**强提示词,而不是只补一句修正提示。
/// 模型被客户端格式带偏时(例如把客户端要的 JSON 未转义塞进 content),只补修正会继续跑偏。
#[tokio::test]
async fn parse_failure_retry_reinjects_the_full_prompt() {
    let (agent, backend) = harness(vec!["这不是 JSON", FINAL_DONE]);
    let outcome = agent.run(build_run("随便问", &[], 8)).await.unwrap();
    assert_eq!(outcome.content, "done");

    let sent = backend.sent();
    let retry = sent.last().expect("至少发出过一条消息");
    assert!(
        retry.contains("工具调用代理"),
        "重试载荷必须带上完整强提示词:{retry}"
    );
    assert!(retry.contains("【规则】"), "重试载荷必须带上完整规则");
    assert!(retry.contains("无法解析"), "重试载荷必须仍带修正提示");
    assert!(
        retry.contains("这不是 JSON"),
        "修正提示里要回显上次原文,便于模型自查"
    );
}

#[tokio::test]
async fn rotation_reinjects_prompt_when_context_full() {
    let (agent, backend) = harness(vec![
        TOOL_CALL_TIME,
        r#"{"type":"final","content":"摘要:已查过时间"}"#,
        FINAL_DONE,
    ]);
    backend.set_context_chars(9_999_999); // 触发 ContextManager::decide 的 Rotate 分支
    let outcome = agent
        .run(build_run("现在几点", &["get_time"], 8))
        .await
        .unwrap();
    assert_eq!(outcome.content, "done");
    assert_eq!(outcome.rotations.len(), 1, "应轮换一次到新对话");
    let injections = backend.injections();
    assert_eq!(
        injections.len(),
        2,
        "轮换后强提示词应在新对话首条消息里再注入一次"
    );
    assert!(
        injections[1].contains("历史摘要"),
        "新对话首条消息应带上上一段对话的摘要"
    );
}

#[tokio::test]
async fn max_rounds_exceeded_returns_error() {
    let (agent, _backend) = harness(vec![TOOL_CALL_TIME, TOOL_CALL_TIME, TOOL_CALL_TIME]);
    let err = agent
        .run(build_run("无限工具", &["get_time"], 2))
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::MaxRoundsExceeded(2)), "应按 max_rounds 截断,实际:{err}");
}

#[tokio::test]
async fn unlisted_tool_is_rejected_and_retried() {
    let (agent, _backend) = harness(vec![
        r#"{"type":"tool_call","tool":"nope","arguments":{}}"#,
        FINAL_DONE,
    ]);
    let outcome = agent.run(build_run("用个没授权的工具", &["get_time"], 8)).await.unwrap();
    assert_eq!(outcome.content, "done");
    let first = outcome.steps.first().expect("至少一步");
    assert_eq!(first.tool_ok, Some(false), "未授权工具应记为 ok=false");
}
