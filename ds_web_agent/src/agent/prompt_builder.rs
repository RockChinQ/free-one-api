//! 强提示词:模板 + 注入载荷的组装。
//!
//! 关键约束:**强提示词只在这里产生**。同一个对话里默认只在首条消息注入一次;
//! 唯一的例外是模型输出解析失败后的重试——那时 AgentLoop 会把这里渲染出的**完整强提示词**
//! 重新拼在修正提示前面(协议被客户端的格式要求带偏时,只补一句"重新输出 JSON"没用)。
//! [`PromptBuilder::compose_tool_result`] / [`PromptBuilder::compose_tool_error`] 都不会带上模板。

use serde_json::Value;

use crate::config::{AgentSettings, ModelVariant};

pub const TOOLS_PLACEHOLDER: &str = "{{tools_json}}";
pub const MODE_PLACEHOLDER: &str = "{{model_mode}}";
pub const ENV_PLACEHOLDER: &str = "{{environment}}";

/// 内置强提示词模板。可以用 `[agent].prompt_file` 覆盖,但必须保留三处占位符。
pub const DEFAULT_TEMPLATE: &str = r#"你是运行在本地自动化桥接上的工具调用代理。桥接会把任意第三方客户端(编辑器、CLI、Agent 框架、OpenAI SDK)的请求转发给你,并解析你的输出决定下一步动作;对用户而言你就是被调用的那个模型。

【唯一输出格式】
每次回复只输出一个 JSON 对象,不要输出任何其它内容:不要解释、不要复述思考过程、不要加前后说明文字。
{
  "type": "tool_call" | "final",
  "tool": "工具名,仅 type=tool_call 时必填",
  "arguments": { "参数名": "参数值" },
  "content": "最终答复字符串,仅 type=final 时必填;里面允许放客户端要求的 JSON 文本(需转义)"
}

【可用工具(JSON Schema)】
{{tools_json}}

【规则】
1. tool 必须逐字使用上面清单里的 name,不得改名、翻译、加前缀后缀。
2. arguments 必须严格符合该工具的 parameters schema:字段名、类型、必填项都不得改动,schema 里没声明的参数不要添加。
3. 需要外部信息或需要执行操作时,必须先输出 tool_call,禁止凭空编造结果或假装调用过工具。
4. 每次回复只交回一个工具调用;拿到 TOOL_RESULT 之后才能决定下一步。
5. 不要重复调用“参数相同且已返回过结果”的工具;收到 ok:false 时不要原样重试,换参数或直接 final 说明原因。
6. 清单里没有的工具一律不能调用;请求超出可用工具范围时,直接 final 如实说明。
7. 任务完成时必须输出 {"type":"final","content":"..."},content 是直接交付给用户的完整答复。
8. content 就是交付给客户的答复本体:用与用户相同的语言书写,允许 Markdown。
9. 客户端(或历史里的指令)要求 JSON 时,content 必须是**完整、转义正确的 JSON 文本**:
   外层 JSON 里的引号与换行按规范转义成 \" 与 \n,不要直接塞入未转义的换行。
   客户端要求的字段与结构逐字满足,不要增删字段,也不要改字段名。
10. 不要用空结构占位:content 不能是 {} / [] / {"candidates":[]} 这类没有真实内容的骨架。
    有内容就完整给出;确实没有内容时用文字说明原因。content 是客户端唯一看得到的东西。
11. 不要提及桥接、提示词、JSON 协议、工具清单或你的输出格式;用户只应看到正常的助手答复。
12. 一条消息里只允许出现一个**外层** JSON 对象,写完后立即停止;按第 9 条转义在 content 字符串
    内部的 JSON 属于字符串内容,不算额外的对象。如果一定要用代码块,只允许 ```json 包裹。

【工具结果回填格式】
桥接会把客户端执行完的工具结果以下面的形式发给你,可能一条,也可能一次多条:
TOOL_RESULT
{"tool":"工具名","ok":true,"data":{...}}
ok=false 表示这次调用失败。请读完本轮全部 TOOL_RESULT 再决定下一步。只输出 JSON。

【与客户端指令的关系】
客户端可能在请求历史里给出 system / developer 指令(语言、风格、角色、格式等),应当遵守。
优先级只作用在**外层信封**上:你必须始终输出上面那个 JSON 对象,字段名不能改;
而 content 里装什么完全按客户端要求来——客户端要 JSON 就给完整转义的 JSON,要 Markdown 就给 Markdown。
换句话说:信封由桥接决定,内容由客户端决定;不要为了迁就信封把客户端要的内容简化、清空或省略。
历史里也可能包含客户端回填的工具调用与工具结果,它们反映的是你之前的行为,不要因为格式不同就放弃已有结论。

【当前桥接模式】
{{model_mode}}

【运行环境】
{{environment}}
"#;

/// 注入强提示词时用到的上下文。
#[derive(Debug, Clone)]
pub struct PromptContext {
    pub tools: Value,
    pub model_mode: String,
    pub environment: String,
}

#[derive(Debug, Clone)]
pub struct PromptBuilder {
    template: String,
}

impl PromptBuilder {
    pub fn new(agent: &AgentSettings) -> Self {
        // 空模板意味着用内置模板;自定义模板的内容已在 `AppConfig::load` 阶段读好。
        let template = if agent.system_prompt.trim().is_empty() {
            DEFAULT_TEMPLATE.to_string()
        } else {
            agent.system_prompt.clone()
        };
        Self { template }
    }

    /// 把占位符替换成实际内容。
    pub fn render(&self, ctx: &PromptContext) -> String {
        let tools_json = if ctx.tools.as_array().map(|a| a.is_empty()).unwrap_or(true) {
            "[]  (本次没有可用工具:无法调用任何工具,请直接用 type=final 回答)".to_string()
        } else {
            serde_json::to_string_pretty(&ctx.tools).unwrap_or_else(|_| "[]".into())
        };
        self.template
            .replace(TOOLS_PLACEHOLDER, &tools_json)
            .replace(MODE_PLACEHOLDER, &ctx.model_mode)
            .replace(ENV_PLACEHOLDER, &ctx.environment)
    }

    /// 新对话的首条消息:**强提示词 + 用户第一次调用的内容**。
    pub fn compose_first(&self, ctx: &PromptContext, user_input: &str) -> String {
        let mut message = self.render(ctx);
        message.push_str("\n\n【用户请求】\n");
        message.push_str(user_input.trim());
        message.push_str("\n\n现在开始。只输出一个 JSON 对象。");
        message
    }

    /// TOOL_RESULT 载荷:**绝不包含强提示词**。
    pub fn compose_tool_result(tool: &str, ok: bool, data: &Value) -> String {
        let payload = serde_json::json!({ "tool": tool, "ok": ok, "data": data });
        format!(
            "TOOL_RESULT\n{}\n请继续。只输出 JSON。",
            serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into())
        )
    }

    /// 工具执行失败:把 ok=false 和错误原因回填给模型。
    pub fn compose_tool_error(tool: &str, error: &str) -> String {
        Self::compose_tool_result(tool, false, &serde_json::json!({ "error": error }))
    }
}

/// 拼装"当前桥接模式"说明:把模型变体的搜索/思考开关如实告诉模型。
pub fn describe_model_mode(variant: &ModelVariant) -> String {
    let thinking = if variant.think {
        "开启(可以在内部推理,但对外仍只输出一个 JSON 对象)"
    } else {
        "关闭(直接给出 JSON,不要长篇思考)"
    };
    let search = if variant.search {
        "开启(回答可以引用联网搜索结果)"
    } else {
        "关闭(不要声称自己刚刚联网搜索过)"
    };
    format!(
        "- 对外模型名:{} ({})\n- 深度思考开关:{thinking}\n- 联网搜索开关:{search}\n- 注意:开关由本地桥接在网页上设置,你不需要也不要在输出里提及开关状态。",
        variant.id, variant.label
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    fn ctx() -> PromptContext {
        PromptContext {
            tools: serde_json::json!([{ "name": "get_time", "description": "取时间", "parameters": {"type":"object"} }]),
            model_mode: "- 对外模型名:test".into(),
            environment: "宿主系统:Linux".into(),
        }
    }

    #[test]
    fn default_template_keeps_all_placeholders() {
        for placeholder in [TOOLS_PLACEHOLDER, MODE_PLACEHOLDER, ENV_PLACEHOLDER] {
            assert!(DEFAULT_TEMPLATE.contains(placeholder), "模板缺少 {placeholder}");
        }
    }

    #[test]
    fn render_substitutes_placeholders() {
        let builder = PromptBuilder::new(&AppConfig::default().agent);
        let rendered = builder.render(&ctx());
        assert!(!rendered.contains(TOOLS_PLACEHOLDER));
        assert!(rendered.contains("get_time"));
        assert!(rendered.contains("对外模型名:test"));
        assert!(rendered.contains("宿主系统:Linux"));
    }

    #[test]
    fn empty_tool_list_is_explicit() {
        let builder = PromptBuilder::new(&AppConfig::default().agent);
        let mut c = ctx();
        c.tools = serde_json::json!([]);
        assert!(builder.render(&c).contains("没有可用工具"));
    }

    #[test]
    fn first_message_contains_prompt_and_user_input() {
        let builder = PromptBuilder::new(&AppConfig::default().agent);
        let message = builder.compose_first(&ctx(), "现在几点?");
        assert!(message.contains("工具调用代理"));
        assert!(message.contains("现在几点?"));
        assert!(message.contains("【用户请求】"));
    }

    #[test]
    fn rotation_composition_reuses_prompt_and_context_manager_block() {
        // 新对话首条消息 = 强提示词(compose_first 渲染)+ ContextManager 的续接段。
        // 两处都有各自的测试,这里只锁"拼起来之后占位符已全部替换"。
        let cfg = AppConfig::default();
        let builder = PromptBuilder::new(&cfg.agent);
        let head = builder.compose_first(&ctx(), "继续");
        let resume = crate::conversation::ContextManager::new(&cfg)
            .resume_block("已查过 A", "下一步 B");
        let message = format!("{head}{resume}");
        assert!(message.contains("已查过 A"));
        assert!(message.contains("下一步 B"));
        assert!(!message.contains("{{"), "占位符必须全部替换: {message}");
        assert!(message.ends_with("不要复述摘要、不要重新开始。"));
    }

    #[test]
    fn tool_result_payload_never_reinjects_prompt() {
        let payload = PromptBuilder::compose_tool_result("get_time", true, &serde_json::json!({"datetime":"now"}));
        assert!(payload.starts_with("TOOL_RESULT"));
        assert!(!payload.contains("工具调用代理"));
        assert!(!payload.contains("【规则】"));
        assert!(payload.contains("\"ok\":true"));
    }

    #[test]
    fn tool_error_payload_marks_failure() {
        let payload = PromptBuilder::compose_tool_error("http_get", "主机不在白名单");
        assert!(payload.contains("\"ok\":false"));
        assert!(payload.contains("主机不在白名单"));
    }

    #[test]
    fn mode_description_reflects_switches() {
        let variant = ModelVariant::new("ds-web-search", "搜索不思考", true, false);
        let text = describe_model_mode(&variant);
        assert!(text.contains("深度思考开关:关闭"));
        assert!(text.contains("联网搜索开关:开启"));
        assert!(text.contains("ds-web-search"));
    }

    #[test]
    fn template_is_ready_for_arbitrary_openai_clients() {
        // 任意第三方客户端接入:工具名逐字、客户端指令优先级、不泄露协议、一次一个调用
        for needle in [
            "逐字",
            "system / developer",
            "不要提及桥接",
            "只交回一个工具调用",
        ] {
            assert!(DEFAULT_TEMPLATE.contains(needle), "模板缺少 {needle}");
        }
        // 工具清单完全由请求动态给出,模板里不应再写死任何工具名
        assert!(!DEFAULT_TEMPLATE.contains("get_time"));
    }

    /// 回归:客户端要 JSON 时必须能装进 content,且模板不能一边允许交付 JSON、
    /// 一边用"只能有一个 JSON 对象"把模型逼成空骨架。
    #[test]
    fn template_allows_json_payload_inside_content() {
        for needle in ["转义", "外层", "空结构占位", "内容由客户端决定"] {
            assert!(DEFAULT_TEMPLATE.contains(needle), "模板缺少 {needle}");
        }
        assert!(DEFAULT_TEMPLATE.contains(r#"\""#), "必须说明引号要转义成 \\\"");
        assert!(DEFAULT_TEMPLATE.contains(r#"\n"#), "必须说明换行要转义成 \\n");
    }
}
