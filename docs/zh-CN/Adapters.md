# 适配器

Free One API 目前支持多种 LLM 逆向工程库，每个渠道支持对应一个适配器，适配器负责将客户端的请求转换成逆向工程库的请求，并将逆向工程库的响应转换成客户端的响应。

## acheong08/ChatGPT

ChatGPT 官网逆向工程库

### 配置方式

1. 选择 `acheong08/ChatGPT` 作为 `Adapter`

![选择适配器](assets/select_adapter.png)

2. 前往 `chat.openai.com` 登录账号

3. 直接在浏览器访问 `https://chat.openai.com/api/auth/session`, 复制得到的 `access_token`

![Alt text](assets/get_actoken.png)

4. 在 `Config` 栏中输入

```json
{
  "access_token": "你的access token"
}
```

5. 保存即可测试

### 反向代理

ChatGPT 需要通过反向代理才能绕过 Cloudflare 的限制，Free One API 项目默认使用开发者提供的代理地址 `https://chatproxy.rockchin.top/api/` ，但压力很大，强烈建议自行搭建反向代理。

* 可以根据以下项目搭建，若均不可用，请自行寻找其他反代:
  - https://github.com/flyingpot/chatgpt-proxy （推荐）
  - https://github.com/acheong08/ChatGPT-Proxy-V4 （不可用）

在`data/config.yaml`中修改`adapters.acheong08_ChatGPT.reverse_proxy`为你的反向代理地址即可。
也可以在创建 `acheong08/ChatGPT` 适配器时，直接在 `Config` 栏中输入

```json
{
  "reverse_proxy": "你的反向代理地址"
}
```

设置此适配器使用的反向代理地址。

> **WARNING**  
> 目前使用的反代可能存在[重复回复前文的情况](https://github.com/RockChinQ/free-one-api/issues/75)，`free-one-api` 会自动删除重复内容，若出现意想不到的情况，请设置`adapters.acheong08_ChatGPT.auto_ignore_duplicated`为`false` 以禁用此功能。

## KoushikNavuluri/Claude-API

Anthropic Claude 官网逆向工程库

### 配置方式

1. 选择 `KoushikNavuluri/Claude-API` 作为 `Adapter`

2. 登录 `claude.ai`, 打开 `F12`, 选择 `Network` 栏, 找到任意一个请求, 复制请求头中的 `Cookie` 字符串

![claude_get_cookie](assets/claude_cookie.png)

3. 在 `Config` 栏中输入

```json
{
  "cookie": "你的cookie"
}
```

## xtekky/gpt4free

xtekky/gpt4free 整合了多个平台的 LLM 逆向工程库

### 配置方式

1. 选择 `xtekky/gpt4free` 作为 `Adapter`

2. 无需鉴权，直接保存即可

## Soulter/hugging-chat-api

huggingface.co/chat 官网逆向工程库

### 配置方式

1. 注册 `HuggingFace` 账号

2. 选择 `Soulter/hugging-chat-api` 作为 `Adapter`

3. 在 `Config` 栏中输入

```json
{
  "email": "HuggingFace 注册邮箱",
  "passwd": "HuggingFace 密码"
}
```

## xw5xr6/revTongYi

阿里云通义千问官网逆向工程库

### 配置方式

1. 选择 `xw5xr6/revTongYi` 作为 `Adapter`

2. 前往 <https://qianwen.aliyun.com/> 登录账号并确保可以正常对话

3. 参照上方 Claude 的配置方式，获取 `Cookie` 字符串

4. 在 `Config` 栏中输入

```json
{
  "cookie": "通义千问cookie"
}
```

## Zai-Kun/reverse-engineered-chatgpt

ChatGPT 官网逆向工程库，这个新一点。acheong08/ChatGPT 已经不维护了。

### 配置方式

1. 选择 `Zai-Kun/reverse-engineered-chatgpt` 作为 `Adapter`

2. 前往 `chat.openai.com` 登录账号

3. 从 Cookies 中取出 `__Secure-next-auth.session-token` 的值作为 `session_token`

4. 在 `Config` 栏中输入

```json
{
  "session_token": "你的session token"
}
```

## ds-webagent/deepseek-web

把本地运行的 DS_WebAgent 接成一个渠道。DS_WebAgent 是仓库里 ds_web_agent/ 目录下的 Rust 桥梁：它驱动一个真实的（有头）浏览器操作 chat.deepseek.com，对外暴露 OpenAI 兼容接口，因此不需要任何 cookie / session token，但需要你先在那个浏览器里手动登录一次 DeepSeek。

### 部署方式

用仓库根目录的 docker-compose.dswebagent.yaml 一起启动（推荐）：

```bash
docker compose -f docker-compose.dswebagent.yaml up -d --build
```


启动后：

1. 打开 http://localhost:16080/vnc.html，在容器里的 Chromium 中手动登录 DeepSeek（登录态持久化在 ./ds_webagent-data/browser-profile，之后重启容器不用再登）；
2. 打开 http://localhost:3000/ 进入本项目管理页（默认密码 12345678）；
3. 「添加渠道」→ 适配器选 ds-webagent/deepseek-web → 保存 → 点测试。

### 配置方式

```json
{
  "base_url": "http://ds-webagent:9178",
  "api_token": "local-dev-token",
  "default_model": "ds-web-search",
  "timeout_secs": 900
}
```


| 字段 | 说明 |
|---|---|
| base_url | DS_WebAgent 的地址。同一 compose 网络里用 http://ds-webagent:9178；非 Docker 部署时用 http://127.0.0.1:9178 |
| api_token | 必须与 DS_WebAgent 的 [server].api_token 一致，否则 401 |
| default_model | 客户端请求的模型名不在下面三个里时的兜底 |
| timeout_secs | 单次请求超时（网页对话较慢，默认 900 秒） |

### 模型名

DS_WebAgent 只认三个模型 ID，对应网页上「联网搜索 / 深度思考」两个开关的组合：

| model id | 联网搜索 | 深度思考 |
|---|---|---|
| ds-web-search | 开 | 关 |
| ds-web-think | 关 | 开 |
| ds-web-search-think | 开 | 开 |

客户端传别的模型名（如 gpt-4o）会落到 default_model，不会报错。

### 工具调用（已支持）

请求里的 `tools` / `tool_choice` 会原样转发给 DS_WebAgent；模型要调工具时，本渠道返回
`finish_reason: "tool_calls"` 与 `message.tool_calls`，**工具由调用方执行**：

1. 客户端执行完工具，把 `{"role":"tool","tool_call_id":"...","content":"..."}` 追加到 `messages` 再请求一次；
2. 模型基于工具结果继续，最终返回 `finish_reason: "stop"` 与 `content`。

DS_WebAgent 内部一次只承载一个工具调用，所以 `message.tool_calls` 数组长度为 1。

### 配置热修改

适配器每次请求都重新读取配置：在管理页改完渠道的 `Config` 并保存，下一次请求立即生效，不需要重启容器。

### 已知限制

- DS_WebAgent 内部串行（max_concurrent_requests = 1），并发请求会排队；心跳检测用的 /health 在 DeepSeek 未登录时返回 ready: false，渠道会被判为不可用，属正常现象。
- 渠道的「测试」按钮只探测 DS_WebAgent 的 /health，不会真的发一次对话。
