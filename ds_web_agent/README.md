# DS_WebAgent

把 **DeepSeek 网页对话**封装成本地 HTTP API(OpenAI 兼容外观),并叠加一层工具调用代理。

> 完整使用说明(OpenAI 客户端接入、客户端工具调用协议、配置项、故障排查)见 [`docs/USAGE.md`](docs/USAGE.md)。

- 启动真实、有头的浏览器,打开 `https://chat.deepseek.com/`,**等你手动登录**;
- 登录后,通过页面操作把"新建对话 / 输入 / 发送 / 等待流式结束 / 提取回复"封装成 HTTP 接口;
- API 调用时,首轮注入一次强提示词,要求模型只输出 JSON(`tool_call` / `final`);
- 客户端下一轮只要照常回放我们上一轮的答复,就**沿用同一个网页对话**、只发增量,
  不重复注入(开了新聊天、换模型或历史对不上才新建对话并重新注入一次);
- `tool_call` 在本地执行工具并把结果回填到同一对话,继续多轮,直到 `final`;
- 上下文接近上限时,自动让当前对话总结,再开新对话、重新注入一次强提示词 + 摘要;
- 同一对话内强提示词只在首条消息注入一次;**解析失败重试时会带上完整强提示词重新注入**。

> 仅本地个人使用、低频串行。不逆向 DeepSeek 私有接口、不绕过验证码/风控/Cloudflare、不伪造浏览器指纹、不隐藏 WebDriver 特征。

## 架构

```
HTTP(axum) ── AgentLoop ── ConversationAdapter ── BrowserAdapter(chromiumoxide) ── 真实浏览器
     │            │              │
     │            ├─ PromptBuilder(强提示词 + JSON 协议)
     │            ├─ json_parser(代码块 / 大括号截取)
     │            └─ ToolRegistry(白名单 / 超时 / 参数校验)
     └─ Store(SQLite + JSONL,使用量统计)
```

每一层都可以单独替换:`BrowserAdapter` 是 trait,换 `headless_chrome` 只改 `src/browser/`;`ChatBackend` 是 trait,换 mock 实现只改 `src/conversation/`。

## 环境要求

- Rust stable(本项目用 1.98 验证)。
- 本机装有 Chrome 或 Chromium(Windows / Linux 均可)。若不在 PATH,在 `config.toml` 里设 `[browser].chrome_executable`。
- DeepSeek 账号(用于手动登录)。

## 安装与启动

```bash
git clone <repo> DS_WebAgent
cd DS_WebAgent
cargo run            # 或 cargo run --release
```

首次启动会:
1. 在 `./data/browser-profile/` 建独立 user-data-dir(保留登录态);
2. 打开一个有头浏览器窗口并进入 DeepSeek;
3. 在控制台提示你在该窗口里手动登录。

登录成功后,`/health` 返回 `ready`,即可调用 API。

### 命令行参数

```bash
cargo run -- --config path/to/config.toml   # 指定配置
cargo run -- --no-browser                    # 只起 HTTP,不开浏览器(调试契约用)
cargo run -- --print-config                  # 打印生效配置后退出
```

## 配置

主配置在 `config.toml`,可用环境变量覆盖白名单项(`DSWA_*`):

| 环境变量 | 作用 |
|---|---|
| `DSWA_CONFIG` | 配置文件路径 |
| `DSWA_HOST` / `DSWA_PORT` | 监听地址 |
| `DSWA_API_TOKEN` | Bearer 鉴权 token(空=仅本机不校验) |
| `DSWA_CHROME_EXECUTABLE` | Chrome 可执行文件路径 |
| `DSWA_HEADLESS` | `true` 时无头(默认 false,登录需要可见窗口) |
| `DSWA_LOG_LEVEL` / `RUST_LOG` | 日志级别 |

关键 section:

- `[selectors]`:**所有页面 DOM 选择器都在这里**。DeepSeek 改版导致选择器漂移时,只改本节、不改代码。支持普通 CSS 选择器与 `text=按钮文字` 形式。
- `[models]`:三个模型变体,对应网页上"联网搜索 / 深度思考"两个开关的组合(见下)。
- `[tools]`:工具白名单、危险工具开关、执行超时。
- `[context]`:上下文阈值与摘要上限。
- `[pricing]`:单价,用于折算"相对官方 API 的累计节省金额"。

## 模型(搜索 / 思考开关)

| model id | 联网搜索 | 深度思考 | 含义 |
|---|---|---|---|
| `ds-web-search` | ✅ | ❌ | 搜索不思考 |
| `ds-web-think` | ❌ | ✅ | 思考不搜索 |
| `ds-web-search-think` | ✅ | ✅ | 搜索且思考 |

API 调用时传 `model` 字段选择;**必须是下面三个 ID 之一**,未知模型名返回 `404 model_not_found`。`[models].apply_toggles_on_new_conversation = true` 时,每次新建对话都会按变体重新设置网页上的两个开关。默认变体由 `[models].default` 指定。

## API

模型调用只有一条路径:`/v1/chat/completions`(OpenAI 兼容外观);其余端点都是管理/观测用途。

鉴权:`/health`、`/v1/health`、`/v1/stats` 无需 token;其余接口需要
`Authorization: Bearer <DSWA_API_TOKEN>`(token 为空时表示仅本机使用、不校验)。

### 健康检查

```bash
curl http://127.0.0.1:9178/health
# {"status":"ready","ready":true,...}  # 还没登录时 status=login_required / starting
```

### 用量统计(免 token)

```bash
# 直接用浏览器打开就能看
curl http://127.0.0.1:9178/v1/stats
```

### 模型调用 `/v1/chat/completions`(唯一入口)

```bash
curl -X POST http://127.0.0.1:9178/v1/chat/completions \
  -H "Authorization: Bearer local-dev-token" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "ds-web-search",
    "messages": [{"role":"user","content":"现在几点?"}],
    "tools": [{"type":"function","function":{"name":"get_time"}}]
  }'
```

返回标准 `chat.completion` 结构,并在 `ds_webagent` 字段附带 `steps` / `rotations` / 用量。`stream: true` 时按 OpenAI chunk 格式输出并以 `[DONE]` 收尾。

**工具调用由客户端执行。** 请求里的 `tools` 由调用方定义、调用方执行,不需要在本地注册:

1. 模型决定调用工具时,返回 `finish_reason: "tool_calls"` 与 `message.tool_calls`(`content` 为 `null`),桥梁**不会**自己执行;
2. 客户端执行完工具,把结果以 `{"role":"tool","tool_call_id":"...","content":"..."}` 追加到 `messages` 再请求一次;
3. 模型基于工具结果继续,最终返回 `finish_reason: "stop"` 与 `content`。

多轮所需的上下文全部来自客户端的 `messages`(包括 assistant 的 `tool_calls` 与 `role="tool"` 的结果),所以客户端不需要维护会话 id。工具名可以是任意名字(如 `get_weather`);`tool_choice: "none"` 表示本轮不提供工具。

`message.content` 是要交付给客户端的答复本体:可以是自然语言,也可以是客户端要求的 JSON 文本——
桥接只在**外层**强制 `{"type":"final","content":...}` 信封,字符串里装 JSON 时按规范转义即可。

### 其他端点

- `GET /v1/models` / `GET /v1/models/{id}` — 模型列表 / 单个模型(OpenAI 风格)
- `GET /v1/sessions` — 在线会话与最近会话
- `GET /v1/sessions/{id}` / `DELETE /v1/sessions/{id}` — 会话详情 / 关闭
- `GET /v1/tools` — 本地工具注册表与 schema(仅供查看,模型调用不走这条链)
- `GET /v1/stats` — 使用量统计(对话数、总/有效 token、节省金额),**无需 token**
- `GET /v1/errors` — 最近错误
- `GET /v1/diag` — 页面诊断(选择器漂移时用)

### 标准细节

- 模型名必须是 `GET /v1/models` 里的三个 ID 之一;不认识的模型名返回 `404 model_not_found`,
  不会静默换成别的模型;
- `tool_choice` 支持 `"none"` / `"auto"` / `"required"` 与
  `{"type":"function","function":{"name":"x"}}`(指定函数时必须已在 `tools` 里声明);
- `stream_options: {"include_usage": true}` 时,流式响应在 `[DONE]` 之前多推一个 `choices` 为空、
  只带 `usage` 的分片;分片顺序是「声明角色 → 内容 → finish_reason」,同一次响应里 `id` / `created` 一致;
- 未实现的采样参数(`temperature` / `top_p` / `max_tokens` / `n` / `stop` …)会被接受但忽略;
- 错误体是 `{"error":{"message","type","param","code"}}`,`type` 用 OpenAI 的取值
  (`invalid_request_error` / `authentication_error` / `rate_limit_error` / `overloaded_error` / `api_error`),
  桥梁自己的细分原因放在 `code` 里;请求体不是合法 JSON、路径不存在也返回同样的错误体。

## 工具扩展

实现 `Tool` trait 并注册即可:

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use ds_webagent::tools::{Tool, ToolRegistry};

pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "做一件具体的事" }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "x": { "type": "string" } },
            "required": ["x"],
            "additionalProperties": false
        })
    }
    async fn execute(&self, args: Value) -> ds_webagent::Result<Value> {
        Ok(json!({ "echo": args }))
    }
}

// 在 ToolRegistry::new 之后调用 registry.register(Arc::new(MyTool));
```

模型调用的工具一律由**客户端**声明并执行(见上一节),所以本地工具注册表不再出现在模型调用链路上:
`GET /v1/tools` 只负责把它列出来看,`[tools].enabled` / `[tools].allow_dangerous` 也只影响这个列表。
`ToolRegistry::execute` 的「白名单 → 危险工具开关 → 参数 schema 校验 → 超时执行」这条链保留给内部调用与测试。

## 选择器调整

DeepSeek 改版导致找不到元素时,**只改 `config.toml` 的 `[selectors]`**,不改代码。例如输入框变了:

```toml
[selectors]
chat_input = ["textarea#chat-input", "div[contenteditable='true']"]
```

支持普通 CSS 选择器,也支持 `text=按钮文字` 形式(按可见文本匹配)。`assistant_extract_js` 可填一段自定义提取脚本,占位符 `__SELECTORS__` / `__EXCLUDES__` 会被自动替换。

## 日志与截图

- 日志:`logs/deeptoolbridge.log`(同时输出控制台,可 `[logging].stdout = false` 关闭)。
- 诊断截图:`logs/screenshots/`,在发送失败 / 回复超时 / 选择器找不到时自动落盘,错误信息里会带 URL、标题、截图路径。

## 使用量统计

`/v1/stats` 返回:
- 历史新建对话个数;
- 总 token(强提示词 + 载荷 + 模型返回 JSON + 模型思考 + 工具结果回填);
- 有效 token(仅 API 入参与返回给客户端的内容);
- 累计节省金额(按 `[pricing]` 的百万 token 单价折算有效用量)。

估算口径默认 `chars/4`,可在 `[context].mode` 切到 `cjk_aware`(中日韩字符按 1 token)。

## 测试

```bash
cargo test                       # 单元 + 集成(不含真实浏览器)
DSWA_E2E=1 cargo test --test mock_page   # 本地 mock 页面端到端(需本机有 Chrome)
```

覆盖:JSON 提取(代码块 / 大括号截取 / schema 示例干扰)、工具注册与参数校验、AgentLoop 状态机(tool_call→final、解析失败重试、上下文轮换、max_rounds 截断、未授权工具回填)、用量与金额折算。

## 合规声明

- 只通过浏览器正常 UI 操作页面,不调用任何 DeepSeek 私有接口;
- 不绕过验证码、风控、Cloudflare;不伪造浏览器指纹、不隐藏 WebDriver 特征;
- 低频串行(整个请求期间独占浏览器,`[server].max_concurrent_requests` 默认 1);
- 仅本地个人使用。

## 目录结构

```
src/
  main.rs            入口:起浏览器 + 起 HTTP
  lib.rs             库根
  config.rs          config.toml + 环境变量
  error.rs           统一错误(OpenAI 风格错误体)
  browser/           BrowserAdapter + chromiumoxide 实现 + SelectorConfig
  conversation/      ChatBackend + 浏览器实现 + ContextManager
  agent/             AgentLoop + 强提示词 + JSON 解析
  tools/             ToolRegistry + 内置工具
  api/               axum 路由与请求/响应模型
  storage/           SQLite/JSONL + tracing 初始化
  testing.rs         假浏览器 / 假对话后端 / mock 页面
config.toml
tests/               agent_loop(状态机)、mock_page(端到端)
```
