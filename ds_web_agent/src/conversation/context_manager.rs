//! ContextManager:上下文规模估算与"该不该换新对话"的判定。
//!
//! 阈值口径来自配置:
//! - `max_context_chars`:当前对话累计估算字符数的上限;
//! - `reserve_chars`:留白,避免"刚要发就超了";
//! - `max_rounds_per_conversation`:兜底,防止单对话轮次过多导致响应变慢。
//!
//! 判定与执行分离:`decide()` 只回答"要不要轮换",轮换动作由 ChatBackend 完成
//! (需要用模型自己总结旧对话,这一步必须有真实对话上下文)。

use serde::Serialize;

use crate::config::AppConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum RotateReason {
    /// 上下文估算超出阈值
    ContextFull {
        context_chars: usize,
        threshold: usize,
        reserve: usize,
    },
    /// 单对话轮次过多
    TooManyRounds { rounds: usize, limit: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ContextDecision {
    Continue,
    /// 需要先总结当前对话,再开新对话继续
    Rotate { reason: RotateReason },
}

pub struct ContextManager {
    enabled: bool,
    max_context_chars: usize,
    reserve_chars: usize,
    max_rounds_per_conversation: usize,
    summary_max_chars: usize,
}

impl ContextManager {
    pub fn new(cfg: &AppConfig) -> Self {
        Self {
            enabled: cfg.context.enabled,
            max_context_chars: cfg.context.max_context_chars,
            reserve_chars: cfg.context.reserve_chars,
            max_rounds_per_conversation: cfg.conversation.max_rounds_per_conversation,
            summary_max_chars: cfg.context.summary_max_chars,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 实际可用的上下文预算:阈值减去留白,避免"刚好卡在边界"反复触发轮换。
    fn effective_threshold(&self) -> usize {
        self.max_context_chars
            .saturating_sub(self.reserve_chars)
            .max(1)
    }

    pub fn decide(&self, context_chars: usize, rounds_in_conversation: usize) -> ContextDecision {
        if !self.enabled {
            return ContextDecision::Continue;
        }
        if self.max_rounds_per_conversation > 0
            && rounds_in_conversation >= self.max_rounds_per_conversation
        {
            return ContextDecision::Rotate {
                reason: RotateReason::TooManyRounds {
                    rounds: rounds_in_conversation,
                    limit: self.max_rounds_per_conversation,
                },
            };
        }
        let threshold = self.effective_threshold();
        if context_chars >= threshold {
            return ContextDecision::Rotate {
                reason: RotateReason::ContextFull {
                    context_chars,
                    threshold: self.max_context_chars,
                    reserve: self.reserve_chars,
                },
            };
        }
        ContextDecision::Continue
    }

    pub fn summary_max_chars(&self) -> usize {
        self.summary_max_chars
    }

    /// 让**快要满的这个对话**自己总结本轮任务状态。
    /// 刻意要求走同一套 JSON 协议:模型此时已经吃透协议,换别的格式反而容易说废话。
    pub fn summary_request(&self) -> String {
        format!(
            "上下文即将到达上限,需要切换到新对话继续。请把当前进展压缩成一份可交接的摘要。\n\
             只输出一个 JSON 对象,不要 Markdown、不要解释:\n\
             {{\"type\":\"final\",\"content\":\"<摘要>\"}}\n\
             摘要要求:\n\
             1. 用户目标与已确认的约束;\n\
             2. 已完成的关键步骤与结论(含关键数据/文件名/ID);\n\
             3. 已调用过的工具及其重要结果;\n\
             4. 尚未完成的事项与下一步计划;\n\
             5. 不超过 {} 个字符,只保留继续工作必需的信息。",
            self.summary_max_chars
        )
    }

    /// 新对话首条消息里的续接段:摘要 + 当前任务状态。
    pub fn resume_block(&self, summary: &str, task_state: &str) -> String {
        let mut out = String::new();
        out.push_str("\n\n【历史摘要】(来自上一个对话,这是你唯一的记忆来源,直接采信,不要再要求原始上下文)\n");
        out.push_str(summary.trim());
        if !task_state.trim().is_empty() {
            out.push_str("\n\n【当前任务状态】\n");
            out.push_str(task_state.trim());
        }
        out.push_str("\n\n【继续执行】以上摘要与状态已足够,请直接从下一步继续,不要复述摘要、不要重新开始。");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager(max_context: usize, reserve: usize, max_rounds: usize) -> ContextManager {
        let mut cfg = AppConfig::default();
        cfg.context.max_context_chars = max_context;
        cfg.context.reserve_chars = reserve;
        cfg.conversation.max_rounds_per_conversation = max_rounds;
        ContextManager::new(&cfg)
    }

    #[test]
    fn continues_below_threshold() {
        let m = manager(1000, 100, 10);
        assert_eq!(m.decide(800, 1), ContextDecision::Continue);
    }

    #[test]
    fn rotates_when_reserve_is_eaten_into() {
        // 阈值 1000、留白 100 → 900 就该轮换,不必等到 1000
        let m = manager(1000, 100, 10);
        match m.decide(900, 1) {
            ContextDecision::Rotate {
                reason: RotateReason::ContextFull { context_chars, .. },
            } => assert_eq!(context_chars, 900),
            other => panic!("应触发上下文轮换,实际 {other:?}"),
        }
    }

    #[test]
    fn rotates_on_round_limit_first() {
        let m = manager(100_000, 1_000, 3);
        match m.decide(10, 3) {
            ContextDecision::Rotate {
                reason: RotateReason::TooManyRounds { rounds, limit },
            } => {
                assert_eq!(rounds, 3);
                assert_eq!(limit, 3);
            }
            other => panic!("轮次超限应优先轮换,实际 {other:?}"),
        }
    }

    #[test]
    fn disabled_manager_never_rotates() {
        let mut cfg = AppConfig::default();
        cfg.context.enabled = false;
        let m = ContextManager::new(&cfg);
        assert_eq!(m.decide(usize::MAX, 999), ContextDecision::Continue);
    }

    #[test]
    fn resume_block_carries_summary_and_state() {
        let m = manager(1000, 100, 10);
        let block = m.resume_block("已完成 A、B", "下一步:C");
        assert!(block.contains("已完成 A、B"));
        assert!(block.contains("下一步:C"));
        assert!(block.contains("不要再要求原始上下文"));
    }

    #[test]
    fn summary_request_uses_configured_limit() {
        let m = manager(1000, 100, 10);
        assert!(m.summary_request().contains("2400"));
    }
}
