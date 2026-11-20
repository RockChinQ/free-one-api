# 常见问题

## 部署与运行

### 管理页默认密码是什么？怎么改？

默认 12345678，写在 data/config.yaml 的 router.token。改完重启进程生效。

### 打开 3000 端口是空白页？

前端没构建。手动部署时需要先在 web/ 里 npm install && npm run build，产物在 web/dist，服务从那里读静态文件。用仓库的 Dockerfile 则已经内置了前端构建阶段，不会遇到这个问题。

### 重启后渠道和 Key 都没了？

data/ 目录没挂载到宿主机。容器内路径固定是 /app/data，里面是 free_one_api.db 和 config.yaml。

```bash
docker run -v ~/free-one-api/data:/app/data ...
```


### 端口 3000 被占用？

改 data/config.yaml 的 router.port，或者改 compose 的端口映射（"8080:3000" 这种，容器内不用动）。

### 怎么升级？

```bash
# Docker 镜像
docker compose pull && docker compose up -d
```

# 源码部署
git pull && cd web && npm install && npm run build && cd ..
docker compose -f docker-compose.dswebagent.yaml up -d --build   # 用本仓库 compose 的话


数据库结构升级是自动的，data/ 保持挂载即可。

## 渠道与请求

### 渠道「测试」通过，但真发请求就失败？

「测试」只做适配器自己的 test()（对 DS_WebAgent 来说就是探一次 /health），不代表真能对话。真失败要看 docker logs -f free-one-api，那里会打出上游返回的原始错误。

### 报 No suitable channel found？

选渠道有硬性过滤：渠道必须启用、适配器的 supported_path() 必须等于请求路径（/v1/chat/completions）、模型名必须是该渠道认的名字。检查：

- 渠道是不是被心跳自动禁用了（状态不是绿色）；
- 请求的 model 是不是该渠道认的名字。

前两条都正常时，看错误体里的 `param`：如果写成 `model=xxx`，就是模型名没被渠道接受。

- `ds-webagent/deepseek-web` 与 `aeliusai/aelius-web` 接受任意模型名（不认识的名字落到各自的 default_model），一般不会因为这个报 404；
- 其它适配器只认 `supported_models()` 里的名字，客户端写别的（比如用 Claude 的客户端发 `claude-3-5-sonnet`）就会 404。解决办法是在渠道的 `model_mapping` 里把客户端用的名字映射过来。

### 渠道明明没坏，却被心跳自动禁用了？

`watchdog.heartbeat.fail_limit`（默认 3）次连续失败后自动禁用。旧版本的心跳有个 bug：即使探测成功也会把失败计数加一，于是健康渠道每轮都被判失败、两三轮后就被禁用。现在只有探测失败才累加计数，成功会清零。升级后如果渠道已经变成禁用状态，手动点一下启用即可。

### 网页版改版了，DS_WebAgent 报找不到输入框 / 开关点了没反应？

DeepSeek 改版过几次，已经踩到的两处：

- 输入框的 `id="chat-input"` 没了，现在是 `<textarea name="search">`；
- 输入框下方的开关从「联网搜索」改名叫「智能搜索」（英文 Search / DeepThink）。

这些都在 `ds_web_agent/config.toml` 的 `[selectors]` 里，候选列表按顺序尝试，加一条就行，不用改代码。开关是带 `aria-pressed` 的 chip，状态优先读这个属性。如果状态读不出来，桥接只会告警、不会盲点——盲点会把已经开启的开关点成关闭。

### 心跳把渠道自动禁用了？

watchdog.heartbeat.fail_limit（默认 3）次失败后自动禁用，间隔默认 1800 秒。DS_WebAgent 没登录 DeepSeek 时 /health 返回 ready: false，就会被判失败——先登录，再手动启用渠道。

### 流式（stream: true）不逐字返回？

两个可能：

1. 反向代理开了缓冲。Nginx 加 proxy_buffering off;，Caddy 加 flush_interval -1；
2. 某些适配器本来就不支持流式（见适配器表格里的 Stream 列），它们会把整段内容一次性 yield 出来。

### 为什么回复末尾带了广告？

data/config.yaml 里的 random_ad 默认关闭。如果开着，rate 是出现概率，ad_list 是广告文案列表。

### 支持并发吗？

框架本身支持多渠道路由 + 负载均衡，但每个渠道是否支持并发取决于适配器。DS_WebAgent 内部强制串行（max_concurrent_requests = 1），并发请求会排队。

## DS_WebAgent 相关

### 容器里的 Chromium 我看不到？

它跑在 Xvfb 虚拟显示上，通过 noVNC 暴露：http://localhost:16080/vnc.html。首次必须从这里登录 DeepSeek。

### 登录了一次，重启容器还要登吗？

不用。登录态存在 ./ds_webagent-data/browser-profile（容器内 /data/browser-profile），只要挂载卷不删就一直在。

### /health 一直 login_required？

就是还没登录。打开 noVNC 登录 DeepSeek，登录后 /health 会变 ready。

### 报 address already in use（9178）？

宿主机 9178 已被别的进程占用。改 compose 里 ds-webagent 的宿主机端口（如 "19178:9178"），容器内端口别改，free-one-api 是通过服务名 ds-webagent:9178 访问的。

### 渠道测试报 401？

api_token 和 DS_WebAgent 的 [server].api_token 不一致。

### 渠道测试报「连不上」？

base_url 写错了。同一 compose 网络里要写 http://ds-webagent:9178，不能写 127.0.0.1（那是容器自己）。

### 改了渠道 Config 要重启吗？

不用。这个适配器每次请求都重新读配置，保存后下一次请求立即生效。

### 日志一直刷 chromiumoxide WS Invalid message？

Chromium 的 CDP 消息比 chromiumoxide 库预期的版本新，属于噪音，不影响功能。

### 工具调用怎么用？

请求里带 tools，模型要调工具时返回 finish_reason: "tool_calls"，工具由你执行，然后把 role=tool 的结果放回 messages 再请求一次。完整例子见 DS_WebAgent 接入。

### 一次能并行调多个工具吗？

不能。DS_WebAgent 一次响应只承载一个工具调用，需要多个工具请拆成多轮。

## 合规

本项目只通过网页正常 UI 操作，不逆向私有接口、不绕过验证码/风控、不伪造浏览器指纹。请低频使用，仅作个人用途。
