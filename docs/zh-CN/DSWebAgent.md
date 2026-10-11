# 接入 DS_WebAgent（DeepSeek 网页版）

`ds_web_agent/` 是仓库里附带的一个 **Rust 桥梁**：它启动一个真实的（有头）Chromium，按网页正常 UI 操作 `chat.deepseek.com`，对外暴露 **OpenAI 兼容接口**。

好处是不需要 cookie、不需要 session token、不碰任何私有接口；代价是你需要在那只浏览器里 **手动登录一次** DeepSeek。

## 架构

```
OpenAI 客户端
   │  POST /v1/chat/completions
   ▼
free-one-api  (Python, 端口 3000)
   │  渠道适配器 ds-webagent/deepseek-web  —— 原样转发
   ▼
DS_WebAgent  (Rust, 容器内端口 9178)
   │  驱动真实 Chromium
   ▼
chat.deepseek.com 网页版
```

两个进程都跑在容器里，容器之间用服务名互访（`http://ds-webagent:9178`），不需要走宿主机端口。

## 一键部署

仓库根目录的 `docker-compose.dswebagent.yaml`：

```yaml
services:
  ds-webagent:
    build:
      context: ./ds_web_agent
    image: ds-webagent:local
    container_name: ds-webagent
    restart: always
    shm_size: "1gb"                 # Chromium 需要更大的 /dev/shm
    environment:
      DSWA_API_TOKEN: local-dev-token
    volumes:
      - ./ds_webagent-data:/data    # 登录态、日志、统计都在这里
    ports:
      - "19178:9178"                # 宿主机 19178 -> 容器 9178
      - "16080:6080"                # noVNC 可视化桌面

  free-one-api:
    build:
      context: .
    container_name: free-one-api
    restart: always
    depends_on:
      - ds-webagent
    volumes:
      - ./data:/app/data
    ports:
      - "3000:3000"
```

```bash
docker compose -f docker-compose.dswebagent.yaml up -d --build
```

第一次构建要编译 Rust 依赖，大约 5~8 分钟；之后改 Python 代码只会重建 `free-one-api`，很快。

> 宿主机端口用 19178 / 16080 是为了避开常见的本地 9178 占用。如果你本机没占用，把 `ports` 改成 `"9178:9178"`、`"6080:6080"` 更直观。

## 首次登录 DeepSeek

容器里的 Chromium 是「有头」的，但跑在虚拟显示器 Xvfb 上，通过 noVNC 让你能看到并操作它：

1. 浏览器打开 `http://localhost:16080/vnc.html`；
2. 点 **Connect**，你会看到容器里的 Chromium 已经打开了 `chat.deepseek.com/sign_in`；
3. 在里面正常登录 DeepSeek（账号密码 / 扫码都行）；
4. 登录后回到 `http://localhost:19178/health`，应当看到：

```json
{ "status": "ready", "ready": true, "detail": "已登录,可以调用 /v1/chat/completions" }
```

登录态保存在 `./ds_webagent-data/browser-profile`，**之后重启容器不用再登**。

> 不想开 noVNC：把 compose 里的 `16080:6080` 去掉，或者设环境变量 `DSWA_ENABLE_VNC=false`。但这样就没法手动登录了，所以首次登录必须开着。

## 添加渠道

1. 打开 `http://localhost:3000/`，用管理密码（默认 `12345678`）登录；
2. 「渠道」页 → **添加渠道**；
3. 适配器选 **`ds-webagent/deepseek-web`**；
4. `Config` 填：

```json
{
  "base_url": "http://ds-webagent:9178",
  "api_token": "local-dev-token",
  "default_model": "ds-web-search",
  "timeout_secs": 900
}
```

5. 保存 → 点该渠道的 **测试**（探测 `/health`）→ 状态变绿即可；
6. 「API Key」页创建一个 key，客户端用它访问 `http://localhost:3000/v1`。

### 配置项说明

| 字段 | 必填 | 默认 | 说明 |
|---|---|---|---|
| `base_url` | 否 | `http://127.0.0.1:9178` | DS_WebAgent 地址。同 compose 网络用 `http://ds-webagent:9178`；本机裸跑用 `http://127.0.0.1:9178` |
| `api_token` | 否 | `local-dev-token` | 必须与 DS_WebAgent 的 `[server].api_token` 一致，否则 401 |
| `default_model` | 否 | `ds-web-search` | 客户端传了不认识的模型名时兜底用哪个变体 |
| `timeout_secs` | 否 | `900` | 单次请求超时。网页版对话慢，别调太小 |

### 配置热修改

适配器**每次请求都重新读配置**：在管理页改完渠道 `Config` 点保存，下一次请求立即用新地址 / 新 token，**不需要重启容器**。

## 模型名

DS_WebAgent 只认三个模型 ID，对应网页上「联网搜索 / 深度思考」两个开关的组合：

| model id | 联网搜索 | 深度思考 | 说明 |
|---|---|---|---|
| `ds-web-search` | ✅ | ❌ | 搜索不思考 |
| `ds-web-think` | ❌ | ✅ | 思考不搜索 |
| `ds-web-search-think` | ✅ | ✅ | 搜索且思考 |

客户端传别的模型名（例如 `gpt-4o`）不会报错，会落到 `default_model`。

## 工具调用

工具由**调用方**声明、**调用方**执行，跟 OpenAI 官方行为一致：

1. 请求里带上 `tools`（`tool_choice` 可选），本渠道原样转发给 DS_WebAgent；
2. 模型决定调工具时，返回 `finish_reason: "tool_calls"`，`message.content` 为 `null`，工具调用在 `message.tool_calls`；
3. 你执行完工具，把结果以 `{"role":"tool","tool_call_id":"...","content":"..."}` 追加到 `messages` 再请求一次；
4. 模型基于工具结果继续，最终返回 `finish_reason: "stop"` 与 `content`。

第一轮请求：

```bash
curl http://localhost:3000/v1/chat/completions   -X POST   -H "Content-Type: application/json"   -H "Authorization: Bearer 你的key"   -d '{
    "model": "ds-web-search",
    "messages": [{"role": "user", "content": "上海现在天气怎么样？"}],
    "tools": [{
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "查询某个城市的天气",
        "parameters": {
          "type": "object",
          "properties": {"city": {"type": "string"}},
          "required": ["city"]
        }
      }
    }],
    "tool_choice": "auto"
  }'
```

模型要调工具时返回：

```json
{
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "id": "call_xxx",
        "type": "function",
        "function": {"name": "get_weather", "arguments": "{"city": "上海"}"}
      }]
    },
    "finish_reason": "tool_calls"
  }]
}
```

第二轮把工具结果带回去：

```bash
curl http://localhost:3000/v1/chat/completions   -X POST   -H "Content-Type: application/json"   -H "Authorization: Bearer 你的key"   -d '{
    "model": "ds-web-search",
    "messages": [
      {"role": "user", "content": "上海现在天气怎么样？"},
      {"role": "assistant", "content": null, "tool_calls": [{
        "id": "call_xxx", "type": "function",
        "function": {"name": "get_weather", "arguments": "{"city": "上海"}"}
      }]},
      {"role": "tool", "tool_call_id": "call_xxx", "name": "get_weather", "content": "{"temp": 22, "desc": "晴"}"}
    ],
    "tools": [{"type": "function", "function": {"name": "get_weather", "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}}}]
  }'
```

流式（`stream: true`）时，`tool_calls` 会出现在 SSE 分片的 `choices[0].delta.tool_calls` 里，`finish_reason` 同样是 `"tool_calls"`。

### 限制

- 一次响应**只承载一个工具调用**（DS_WebAgent 内部结构决定），需要并行多工具请拆成多轮；
- 参数会按 JSON 解析后交还，`arguments` 是字符串形式的 JSON（与 OpenAI 一致）。

## 端口冲突

如果宿主机 9178 已被占用（例如你本地也在跑一个 DS_WebAgent），compose 会报：

```
failed to bind host port 0.0.0.0:9178/tcp: address already in use
```

改 `docker-compose.dswebagent.yaml` 里的宿主机端口即可（容器内端口不要改，`free-one-api` 走服务名访问）：

```yaml
    ports:
      - "19178:9178"
      - "16080:6080"
```

## 故障排查

| 现象 | 原因 / 处理 |
|---|---|
| `/health` 一直 `login_required` | 还没登录。开 noVNC 登录 DeepSeek，登录后再刷 |
| 渠道测试报 `401` | `api_token` 与 DS_WebAgent 的 `[server].api_token` 不一致 |
| 渠道测试报 `连不上` | `base_url` 写错。同 compose 网络写 `http://ds-webagent:9178`，别写 `127.0.0.1` |
| 请求很慢 / 排队 | DS_WebAgent 内部串行（`max_concurrent_requests = 1`），并发会排队，属预期 |
| 日志刷 `chromiumoxide WS Invalid message` | Chromium 版本比库预期新，噪音，不影响功能 |
| 容器起来但 Chromium 崩 | `/dev/shm` 太小。保留 `shm_size: "1gb"`，配置里已有 `--disable-dev-shm-usage` |
| 点测试通过但真请求失败 | 测试只探 `/health`。确认 DeepSeek 网页能正常对话（登录没掉、没弹验证码） |

看日志：

```bash
docker logs -f ds-webagent
docker logs -f free-one-api
tail -f ./ds_webagent-data/logs/deeptoolbridge.log
```

## 合规

- 只通过网页正常 UI 操作，不调用任何 DeepSeek 私有接口；
- 不绕过验证码、风控、Cloudflare，不伪造浏览器指纹；
- 低频串行，仅本地个人使用。
