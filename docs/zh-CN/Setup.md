# 部署

本项目是一个 Quart(Python) 服务，默认监听 `3000` 端口，管理页和 API 同端口。

> 只想最快跑起来：跳到 [方式一 Docker](#方式一docker推荐)，两行命令即可。

## 环境要求

| 项目 | 要求 |
|---|---|
| Docker 部署 | Docker 20.10+，可选 Docker Compose v2 |
| 手动部署 | Python 3.10+，Node.js 18+（仅构建前端需要） |
| 内存 | 约 300MB（不含 DS_WebAgent） |
| 数据目录 | 容器内 `/app/data`，必须持久化 |

数据目录里有两样东西，**不要丢**：

- `data/free_one_api.db`：渠道、API Key、日志的 SQLite 数据库；
- `data/config.yaml`：全局配置（首次启动自动生成）。

## 方式一：Docker（推荐）

### 1. 拉取现成镜像

```bash
docker run -d   --name free-one-api   --restart always   -p 3000:3000   -v ~/free-one-api/data:/app/data   rockchin/free-one-api
```

打开 `http://localhost:3000/`，默认管理密码 `12345678`（在 `data/config.yaml` 的 `router.token` 修改）。

### 2. Docker Compose

保存为 `docker-compose.yaml`：

```yaml
services:
  free-one-api:
    image: rockchin/free-one-api
    container_name: free-one-api
    restart: always
    ports:
      - "3000:3000"
    volumes:
      - ./data:/app/data
```

```bash
docker compose up -d
docker compose logs -f          # 看启动日志
docker compose pull && docker compose up -d   # 升级
```

### 3. 从源码自己构建镜像

仓库自带的 `Dockerfile` 已经内置前端构建阶段，**不需要**先在宿主机 `npm run build`：

```bash
git clone https://github.com/RockChinQ/free-one-api.git
cd free-one-api
docker build -t free-one-api:local .
docker run -d --name free-one-api -p 3000:3000   -v ./data:/app/data free-one-api:local
```

> 构建阶段会跑 `npm install` 和 `pip install -r requirements.txt`，首次大约 3~5 分钟。

## 方式二：手动部署

```bash
git clone https://github.com/RockChinQ/free-one-api.git
cd free-one-api

# 1. 构建前端（产物在 web/dist，服务会从这里读静态文件）
cd web && npm install && npm run build && cd ..

# 2. 装后端依赖
pip install -r requirements.txt

# 3. 启动
python main.py
```

启动后同样是 `http://localhost:3000/`。

### 用 systemd 常驻（Linux）

```ini
# /etc/systemd/system/free-one-api.service
[Unit]
Description=Free One API
After=network.target

[Service]
Type=simple
WorkingDirectory=/opt/free-one-api
ExecStart=/usr/bin/python3 main.py
Restart=always
RestartSec=5
User=www-data

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now free-one-api
sudo journalctl -u free-one-api -f
```

## 和 DS_WebAgent 一起部署

如果你要用 DeepSeek 网页版渠道，仓库根目录提供了 `docker-compose.dswebagent.yaml`，一次把两个服务都拉起来：

```bash
docker compose -f docker-compose.dswebagent.yaml up -d --build
```

端口、首次登录、渠道配置见 [DS_WebAgent 接入](/zh-CN/DSWebAgent)。

## 反向代理 / HTTPS

服务本身只跑 HTTP，建议放在 Nginx / Caddy 后面：

```nginx
# Nginx
server {
    listen 443 ssl http2;
    server_name api.example.com;

    ssl_certificate     /etc/letsencrypt/live/api.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/api.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_buffering off;          # 流式响应必须关缓冲
        proxy_read_timeout 1800s;     # 网页版对话可能很慢
    }
}
```

```caddyfile
# Caddy
api.example.com {
    reverse_proxy 127.0.0.1:3000 {
        flush_interval -1
    }
}
```

> `proxy_buffering off` / `flush_interval -1` 是关键：否则 `stream: true` 的 SSE 会被整段缓冲，客户端拿不到逐字输出。

## 安全建议

- 首次部署后立刻改掉 `router.token`（管理密码），并重新生成 API Key；
- 只把 `/v1/chat/completions` 暴露给需要的客户端；管理页 `/api/*` 建议加 IP 白名单或 Basic Auth；
- 容器不要用 `--network host`；
- 定期备份 `data/` 目录。

## 升级

```bash
# Docker
docker compose pull && docker compose up -d

# 源码
git pull && cd web && npm install && npm run build && cd .. && pip install -r requirements.txt
# 然后重启进程/服务
```

数据库结构升级是自动的，`data/` 目录保持挂载即可。

## 排错

| 现象 | 原因 / 处理 |
|---|---|
| 打开 3000 是空白页 | 前端没构建（手动部署时 `web/dist` 不存在）。跑 `cd web && npm run build`，或改用 Docker 镜像 |
| 管理页提示密码错误 | 密码是 `data/config.yaml` 里的 `router.token`，首次启动自动生成 `12345678` |
| 渠道一直不可用 | 点渠道的「测试」看报错；适配器本身不通（缺 cookie / 未登录）会被心跳自动禁用 |
| 流式响应一次性返回 | 反向代理开了缓冲，见上面的 Nginx / Caddy 配置 |
| 重启后配置没了 | `data/` 没挂载到宿主机 |
