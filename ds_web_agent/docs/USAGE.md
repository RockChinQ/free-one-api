# DS_WebAgent 使用文档

把 DeepSeek 网页对话封装成本地 HTTP API(OpenAI 兼容外观),让任意支持 OpenAI 协议的客户端
(编辑器插件、CLI、Agent 框架、SDK)都能把它当成一个普通模型来用。

- 项目路径:`/home/mrcwoods/code/DS_WebAgent`
- 默认监听:`http://127.0.0.1:9178`
- 默认鉴权:`Authorization: Bearer local-dev-token`

---

## 1. 能力边界(先看这一节)

| 能做 | 不能做 |
|---|---|
| 真实浏览器打开 DeepSeek 网页,用户手动登录 | 逆向 DeepSeek 私有接口 |
| 把网页对话暴露成 OpenAI 兼容接口 | 绕过验证码 / 风控 / Cloudflare |
| 把**客户端自己声明并执行**的工具调用转达给网页模型 | 伪造浏览器指纹、隐藏 WebDriver 特征 |
| 上下文快满时自动新开对话并重注强提示词 + 历史摘要 | 高频并发 / 批量轰炸(本机串行、个人低频使用) |

关键一点:**DeepSeek 网页本身没有工具调用能力,它只会输出文本。**
所以工具调用的完整闭环是「网页模型决定调用什么 → 桥接把调用交还给客户端 → 客户端执行 →
客户端把结果发回桥接 → 桥接回填给同一个对话 → 模型继续」。桥接**不会**替你执行客户端声明的工具。

模型调用只有一条路径:OpenAI 兼容的 `POST /v1/chat/completions`。桥接不再提供自有协议的入口,也不替客户端执行工具。

---

## 2. 环境要求与安装

- 操作系统:Linux(Debian 13 已验证)/ Windows / macOS
- Rust stable(`cargo --version` 可用)
- 本机装有 Chrome 或 Chromium(默认自动探测;也可用配置显式指定)
- 能正常访问 `https://chat.deepseek.com/`

```bash
cd /home/mrcwoods/code/DS_WebAgent
cargo build --release
# 产物:target/release/ds_webagent
```

首次登录需要一个可见的浏览器窗口,所以必须在有图形界面的机器上跑(不能用无头)。

---

## 3. 配置

主配置是项目根目录的 `config.toml`,页面选择器也全部集中在这里(页面改版时只改配置,不动代码)。

常用段落:

| 段 | 关键项 | 说明 |
|---|---|---|
| `[server]` | `host` `port` `api_token` `request_timeout_secs` | 监听地址与鉴权;串行队列见 `max_concurrent_requests` |
| `[browser]` | `chrome_executable` `home_url` `user_data_dir` `headless` `user_agent` | 登录态保存在 `user_data_dir`(默认 `./data/browser-profile`) |
| `[conversation]` | `reply_timeout_secs` `stable_secs` `max_rounds_per_conversation` | 等回复超时与"文本稳定"判定 |
| `[agent]` | `max_rounds` `max_rounds_limit` `max_json_retries` `prompt_file` | 单请求最多轮次;可用外部文件覆盖强提示词 |
| `[context]` | `enabled` `max_context_chars` `reserve_chars` | 上下文快满时自动轮换到新对话 |
| `[tools]` | `enabled` `allow_dangerous` | 本地工具白名单(只影响 `GET /v1/tools` 的展示) |
| `[tools.http_get]` | `allowed_hosts` | 出网白名单,为空=禁止一切出网 |
| `[models]` | `default` `[[models.variants]]` | 对外模型 ID 与搜索/思考开关组合 |
| `[storage]` | `dir` `sqlite_path` | 日志、JSONL 明细、SQLite 统计 |
| `[pricing]` | `input_per_million` `output_per_million` | 折算"相对 API 节省金额" |
| `[selectors]` | 全部 | 登录检测、输入/发送、消息读取、开关选择器 |

### 环境变量覆盖

| 环境变量 | 作用 |
|---|---|
| `DSWA_CONFIG` | 配置文件路径(等价于 `--config`) |
| `DSWA_HOST` / `DSWA_PORT` | 监听地址与端口 |
| `DSWA_API_TOKEN` | Bearer token(设为空字符串=仅本机不校验) |
| `DSWA_MAX_CONCURRENT` | 并发上限(默认 1,串行) |
| `DSWA_CHROME_EXECUTABLE` / `CHROME_PATH` | Chrome/Chromium 可执行文件路径 |
| `DSWA_HOME_URL` | 起始页面 |
| `DSWA_USER_DATA_DIR` | 登录态目录 |
| `DSWA_USER_AGENT` | UA 字符串 |
| `DSWA_HEADLESS` / `DSWA_NO_SANDBOX` | 无头 / 关闭沙箱(默认 `false`) |
| `DSWA_REPLY_TIMEOUT_SECS` | 等回复超时 |
| `DSWA_MAX_ROUNDS` / `DSWA_MAX_CONTEXT_CHARS` | 轮次上限 / 上下文阈值 |
| `DSWA_TOOLS` / `DSWA_ALLOW_DANGEROUS` | 本地工具白名单 / 危险工具开关 |
| `DSWA_HTTP_GET_ALLOWED_HOSTS` | `http_get` 出网白名单(逗号分隔) |
| `DSWA_FORCE_SELECTOR_FILE` | 用外部 TOML 整体覆盖选择器 |
| `DSWA_LOG_LEVEL` / `RUST_LOG` | 日志级别 |
| `DSWA_STORAGE_DIR` / `DSWA_SQLITE_PATH` | 日志目录 / SQLite 路径 |

查看最终生效的配置:

```bash
./target/release/ds_webagent --print-config
```

---

## 4. 启动与登录

```bash
./target/release/ds_webagent                      # 使用 ./config.toml
./target/release/ds_webagent -c /path/config.toml # 指定配置
./target/release/ds_webagent --no-browser         # 只起 HTTP,不启动浏览器(调 API 契约用)
```

启动顺序:先起 HTTP 服务 → 再在独立任务里启动浏览器并打开 DeepSeek → 等待你在窗口里**手动登录**。
登录态存在 `user_data_dir`,下次启动通常不再需要登录。

`GET /health` 的状态机:

| `status` | 含义 | HTTP |
|---|---|---|
| `starting` | 浏览器还没就绪(加载中,或选择器漂移) | 503 |
| `login_required` | 已打开页面但未登录,请在弹出的窗口里登录 | 503 |
| `ready` | 已登录,可以调用 `/v1/chat/completions` | 200 |
| `browser_failed` | 浏览器启动或页面加载失败 | 503 |

```bash
curl -s http://127.0.0.1:9178/health
# {"status":"ready","ready":true,"detail":"已登录,可以调用 /v1/chat/completions",...}
```

客户端可以在等待登录期间轮询 `/health`(它不会直接 5xx 报错),而不是连接失败。

---

## 5. 模型调用:OpenAI 兼容接口(唯一入口)

`POST /v1/chat/completions`。任何能配置 `base_url` + `api_key` + `model` 的 OpenAI 客户端都能接。

- `base_url`: `http://127.0.0.1:9178/v1`
- `api_key`: `config.toml` 里的 `server.api_token`(默认 `local-dev-token`)
- `model`: 必须是 `ds-web-search`(搜索不思考)/ `ds-web-think`(思考不搜索)/
  `ds-web-search-think`(搜索且思考)之一,`GET /v1/models` 能列出来

`model` 严格校验:名字不在上面三个里就返回 `404 model_not_found`,不会静默换成别的模型——
客户端沿用 `gpt-4o` 之类的名字会直接报错,把它改成三个 ID 之一即可。

### 5.1 基本对话

```bash
curl -s http://127.0.0.1:9178/v1/chat/completions \
  -H "Authorization: Bearer local-dev-token" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "ds-web-search",
    "messages": [{"role": "user", "content": "用一句话介绍你自己"}]
  }'
```

响应就是标准 `chat.completion`,额外多一个 `ds_webagent` 字段(标准客户端会忽略):

```json
{
  "choices": [{"index": 0, "message": {"role": "assistant", "content": "..."}, "finish_reason": "stop"}],
  "usage": {"prompt_tokens": 16, "completion_tokens": 15, "total_tokens": 31},
  "ds_webagent": {"session_id": "sess-...", "conversation_id": "ds-...", "rounds": 2, "steps": [...], "usage": {...}}
}
```

### 5.2 客户端工具调用(核心用法)

工具由**请求声明、客户端执行**,不需要在桥接里注册。

第一步:声明工具并提问。

```bash
curl -s http://127.0.0.1:9178/v1/chat/completions \
  -H "Authorization: Bearer local-dev-token" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "ds-web-search",
    "messages": [{"role": "user", "content": "北京现在天气如何?请调用工具查询"}],
    "tools": [{
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "查询指定城市的当前天气",
        "parameters": {
          "type": "object",
          "properties": {"city": {"type": "string", "description": "城市名"}},
          "required": ["city"]
        }
      }
    }]
  }'
```

模型决定调用工具时,响应是 `finish_reason: "tool_calls"`、`content: null`:

```json
{
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "id": "call_1e117c79ba6447b6931fe270b072f70a",
        "type": "function",
        "function": {"name": "get_weather", "arguments": "{\"city\":\"北京\"}"}
      }]
    },
    "finish_reason": "tool_calls"
  }]
}
```

注意 `arguments` 是 **JSON 字符串**(与 OpenAI 一致),不是对象。

第二步:客户端执行工具,把结果以 `role: "tool"` 追加到 `messages` 再请求一次。

```bash
curl -s http://127.0.0.1:9178/v1/chat/completions \
  -H "Authorization: Bearer local-dev-token" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "ds-web-search",
    "messages": [
      {"role": "user", "content": "北京现在天气如何?请调用工具查询"},
      {"role": "assistant", "content": null, "tool_calls": [
        {"id": "call_1e117c79ba6447b6931fe270b072f70a", "type": "function",
         "function": {"name": "get_weather", "arguments": "{\"city\":\"北京\"}"}}
      ]},
      {"role": "tool", "tool_call_id": "call_1e117c79ba6447b6931fe270b072f70a",
       "name": "get_weather", "content": "{\"temp_c\":18,\"condition\":\"晴\"}"}
    ]
  }'
```

这一步会返回 `finish_reason: "stop"` 和最终 `content`(上面例子实测得到
「北京当前天气:晴,气温 18°C。」)。如果模型还要调工具,会再次返回 `tool_calls`,循环即可。

Python(OpenAI SDK)等价写法:

```python
from openai import OpenAI

client = OpenAI(base_url="http://127.0.0.1:9178/v1", api_key="local-dev-token")
tools = [{
    "type": "function",
    "function": {
        "name": "get_weather",
        "description": "查询指定城市的当前天气",
        "parameters": {"type": "object",
                       "properties": {"city": {"type": "string"}},
                       "required": ["city"]},
    },
}]
messages = [{"role": "user", "content": "北京现在天气如何?"}]

while True:
    resp = client.chat.completions.create(model="ds-web-search", messages=messages, tools=tools)
    msg = resp.choices[0].message
    messages.append(msg)
    if resp.choices[0].finish_reason != "tool_calls":
        print(msg.content)
        break
    for call in msg.tool_calls:
        result = my_local_tool(call.function.name, call.function.arguments)  # 你自己的实现
        messages.append({"role": "tool", "tool_call_id": call.id, "content": result})
```

**多轮上下文全部来自 `messages`**:桥接会渲染 `system` / `assistant(tool_calls)` / `role=tool`
历史并连同本轮一起交给网页模型,所以客户端不需要维护 `session_id`。每次请求对应网页上一次新对话。

### 5.3 流式

`"stream": true` 时按 OpenAI chunk 格式输出,分片顺序是「声明角色 → 内容 → finish_reason」,
末尾以 `data: [DONE]` 收尾;同一次响应里所有分片的 `id` / `created` / `model` 一致。普通内容:

```
data: {"id":"chatcmpl-...","object":"chat.completion.chunk","created":1700000000,"model":"ds-web-search","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}
data: {"id":"chatcmpl-...","object":"chat.completion.chunk","created":1700000000,"model":"ds-web-search","choices":[{"index":0,"delta":{"content":"你好"},"finish_reason":null}]}
data: {"id":"chatcmpl-...","object":"chat.completion.chunk","created":1700000000,"model":"ds-web-search","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}
data: [DONE]
```

工具调用:

```
data: {...,"choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}
data: {...,"choices":[{"index":0,"delta":{"tool_calls":[{"id":"call_...","type":"function","index":0,"function":{"name":"get_weather","arguments":"{\"city\":\"北京\"}"}}]},"finish_reason":null}]}
data: {...,"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}
data: [DONE]
```

请求里带 `"stream_options": {"include_usage": true}` 时,在 `[DONE]` 之前还会多一个只带用量的分片
(`choices` 是空数组):

```
data: {"id":"chatcmpl-...","object":"chat.completion.chunk","created":1700000000,"model":"ds-web-search","choices":[],"usage":{"prompt_tokens":16,"completion_tokens":15,"total_tokens":31,"prompt_tokens_details":{"cached_tokens":0},"completion_tokens_details":{"reasoning_tokens":0}}}
data: [DONE]
```

网页回复本身就是整段的,所以桥接在模型产出完整回复后才把内容一次性吐出(不会逐字伪造进度)。

### 5.4 字段支持情况

| 字段 | 支持 | 说明 |
|---|---|---|
| `model` | ✅ | 必须是三个模型 ID 之一,否则 `404 model_not_found` |
| `messages[].role` | ✅ | `system` / `user` / `assistant` / `tool` |
| `messages[].content` | ✅ | 字符串或 parts 数组(**只取文本**,图片会被忽略) |
| `messages[].tool_calls` | ✅ | 回放历史时带上,模型能看到自己之前的调用 |
| `messages[].tool_call_id` / `name` | ✅ | `role="tool"` 时用于标识结果属于哪次调用 |
| `tools` | ✅ | 任意名字、任意 schema,由客户端执行 |
| `tool_choice` | ✅ | `"none"` / `"auto"` / `"required"` / `{"type":"function","function":{"name":"x"}}`(x 必须已在 `tools` 里) |
| `stream` | ✅ | 见 5.3 |
| `stream_options.include_usage` | ✅ | 见 5.3 的用量分片 |
| `session_id`(扩展) | ✅ | 传给桥接可复用网页会话;不传就是一次新对话 |
| `max_rounds`(扩展) | ✅ | 覆盖 `[agent].max_rounds` |
| `temperature` / `top_p` / `max_tokens` / `n` / `stop` 等 | ❌ | 网页端不可控,传了会被接受但忽略 |
| 并行工具调用 | ❌ | 每次回复只交回一个 `tool_call` |
| 错误响应 | ✅ | `{"error":{"message","type","param","code"}}`,见 5.6 |

**关于 `content`**:`content` 是要交付给你的答复本体,可以是自然语言(允许 Markdown),也可以是客户端要求的
JSON 文本——桥接只在**外层**强制 `{"type":"final","content":...}` 信封,字符串里装 JSON 时按规范转义即可。
若发现模型总在返回空骨架(如 `{"candidates":[]}`),先看客户端指令里对输出结构的要求是否明确:
强提示词已要求它必须逐字满足客户端要的字段、不得用空结构占位。

### 5.6 错误响应

所有失败路径(包括请求体不是合法 JSON、路径不存在)都返回 OpenAI 形状的错误体,HTTP 状态码与之匹配:

```json
{"error":{"message":"模型 `gpt-4o` 不存在;可用模型见 GET /v1/models","type":"invalid_request_error","param":"model","code":"model_not_found","retryable":false}}
```

| HTTP | `type` | 典型场景 |
|---|---|---|
| 400 | `invalid_request_error` | 参数不对(messages 为空、tool_choice 非法、请求体不是合法 JSON) |
| 401 | `authentication_error` | Bearer token 不匹配 |
| 404 | `invalid_request_error` | 模型不存在(`code=model_not_found`)、路径不存在 |
| 429 | `rate_limit_error` | 串行队列忙(`[server].max_concurrent_requests`) |
| 502 | `api_error` | 浏览器/页面出错(选择器漂移等) |
| 503 | `overloaded_error` | 还没登录 DeepSeek |
| 504 | `api_error` | 等回复超时(`code=reply_timeout`) |

### 5.5 常见客户端怎么填

| 客户端 | 配置 |
|---|---|
| OpenAI SDK / 任意 OpenAI 兼容库 | `base_url=http://127.0.0.1:9178/v1`,`api_key=local-dev-token`,`model=ds-web-search` |
| Continue / Cline / Cursor 等(MCP/自定义 OpenAI Provider) | Provider 选 OpenAI Compatible;Base URL 填 `http://127.0.0.1:9178/v1`;API Key 填 token;模型名任填上面三个 ID 之一 |
| Open WebUI / LobeChat 等自托管前端 | 新增 OpenAI 连接:URL `http://127.0.0.1:9178/v1`,Key = token;把工具交给客户端插件机制执行 |
| 只要对话不要工具 | 不传 `tools` 即可,模型会直接回答 |

---

---

## 6. 其他端点

| 端点 | 说明 |
|---|---|
| `GET /health`、`GET /v1/health` | 就绪状态(无需鉴权) |
| `GET /v1/models` / `GET /v1/models/{id}` | 模型列表 / 单个模型(OpenAI 风格) |
| `GET /v1/tools` | 本地工具注册表与 schema(模型调用不走这条链) |
| `GET /v1/sessions` | 在线会话与最近会话 |
| `GET /v1/sessions/{id}` / `DELETE /v1/sessions/{id}` | 会话详情 / 关闭 |
| `GET /v1/stats` | 使用量统计:对话数、总/有效 token、节省金额(**免鉴权**,浏览器直接打开) |
| `GET /v1/errors` | 最近错误 |
| `GET /v1/diag` | **页面诊断**:当前 URL/标题、输入框与头像元素真实 HTML、登录判定结果(选择器漂移时用) |

---

## 7. 上下文与自动新对话

- 每个网页对话的首条消息注入一次强提示词 + 工具清单 + 客户端请求;同一对话内不再重复注入。
  唯一例外:模型输出解析失败要重试时,会把**完整强提示词 + 修正提示**一起重发
  (协议被客户端格式带偏时,只补一句"重新输出 JSON"往往会继续跑偏)。
- 估算上下文超过 `[context].max_context_chars - reserve_chars`,或超过
  `[conversation].max_rounds_per_conversation` 轮,就自动点「新对话」,在新对话首条消息里重新注入
  强提示词 + 历史摘要 + 当前任务状态,然后继续。
- token 估算模式由 `[context].mode` 决定(`chars_div4` 或 `cjk_aware`)。
- 强提示词可用 `[agent].prompt_file` 指向外部模板覆盖,模板必须保留
  `{{tools_json}}`、`{{model_mode}}`、`{{environment}}` 三处占位符。

---

## 8. 用量与节省金额

每次响应都会统计:

- **总 token**:强提示词 + 载荷 + 模型返回 JSON + 模型思考 + 工具结果回填
- **有效 token**:只算 API 入参(你发给它的内容)与返回给客户端的 `content`
- **节省金额**:按 `[pricing]` 的百万 token 单价折算有效用量

`GET /v1/stats` 给出累计值;明细落在 `logs/records/*.jsonl` 与 SQLite(`data/dswa.db`)。

---

## 9. 故障排查

| 现象 | 原因与处理 |
|---|---|
| `/health` 一直 `login_required` | 在浏览器窗口里手动登录;若已登录仍报此状态,访问 `GET /v1/diag` 看真实 URL 与元素 HTML,然后改 `[selectors]` 里的 `not_logged_in_url_markers` / `logged_in_url_markers` / `login_input` |
| `/health` 一直 `starting` | 页面还在加载或选择器漂移;同样看 `/v1/diag`,对照 `[selectors].chat_input` |
| 报错 `reply_timeout`(等待回复超时) | 网页没在 `[conversation].reply_timeout_secs` 内产出"稳定新回复"。截图存于 `logs/screenshots/`;可调大超时或 `stable_secs` |
| 日志出现 `未找到开关,按页面当前状态继续` | `[selectors].toggle_search` / `toggle_think` 没匹配上,当前对话会沿用页面上的开关状态 |
| 启动报找不到 Chrome | 设置 `DSWA_CHROME_EXECUTABLE` 或 `[browser].chrome_executable` |
| 端口被占用 | 改 `[server].port` 或 `DSWA_PORT` |
| 工具调用没触发 | 请求里要带 `tools`,`tool_choice` 不能是 `"none"`,且模型判断确实需要工具 |
| 报 404 `model_not_found` | `model` 不是 `GET /v1/models` 里的三个 ID 之一,改成一个存在的 ID |
| 401 Unauthorized | `Authorization: Bearer <server.api_token>` 不匹配(`/v1/stats` 与 `/health` 不需要) |

日志:`logs/deeptoolbridge.log`(同时输出到控制台);最近错误看 `GET /v1/errors`,页面状态看 `GET /v1/diag`。

---

## 10. 安全与合规

- 仅本机个人使用,低频串行;`[server].api_token` 为空时表示只监听本机、不校验。
- 只有 `/health`、`/v1/health`、`/v1/stats` 不需要 token;`/v1/stats` 是只读统计,服务也只绑在本机。
- 不逆向 DeepSeek 私有接口、不绕过验证码/风控/Cloudflare、不伪造浏览器指纹与 WebDriver 特征;
  只通过标准启动参数设置 UA(见 `config.toml` 的注释)。
- `http_get` 默认禁止一切出网;放开时必须显式列域名,并仍受内网/元数据地址拦截。
