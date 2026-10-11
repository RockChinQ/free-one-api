# Deployment

This is a Quart (Python) service listening on port `3000` by default; the admin page and the API share that port.

> Just want it running fast? Jump to [Docker](#option-1-docker-recommended) — two commands are enough.

## Requirements

| Item | Requirement |
|---|---|
| Docker | Docker 20.10+, Docker Compose v2 optional |
| Manual | Python 3.10+, Node.js 18+ (only needed to build the frontend) |
| Memory | ~300MB (without DS_WebAgent) |
| Data dir | `/app/data` inside the container, must be persisted |

Two things live in the data directory — **do not lose them**:

- `data/free_one_api.db` — SQLite database with channels, API keys and logs;
- `data/config.yaml` — global config (created automatically on first start).

## Option 1: Docker (recommended)

### 1. Pull the published image

```bash
docker run -d \
  --name free-one-api \
  --restart always \
  -p 3000:3000 \
  -v ~/free-one-api/data:/app/data \
  rockchin/free-one-api
```

Open `http://localhost:3000/`; the default admin password is `12345678` (change `router.token` in `data/config.yaml`).

### 2. Docker Compose

Save as `docker-compose.yaml`:

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
docker compose logs -f                        # follow startup logs
docker compose pull && docker compose up -d   # upgrade
```

### 3. Build the image from source

The repository `Dockerfile` already contains the frontend build stage, so you do **not** need to run `npm run build` on the host first:

```bash
git clone https://github.com/RockChinQ/free-one-api.git
cd free-one-api
docker build -t free-one-api:local .
docker run -d --name free-one-api -p 3000:3000 \
  -v ./data:/app/data free-one-api:local
```

> The build runs `npm install` and `pip install -r requirements.txt`; the first one takes roughly 3–5 minutes.

## Option 2: Manual

```bash
git clone https://github.com/RockChinQ/free-one-api.git
cd free-one-api

# 1. Build the frontend (output lands in web/dist, the server serves it from there)
cd web && npm install && npm run build && cd ..

# 2. Install backend dependencies
pip install -r requirements.txt

# 3. Start
python main.py
```

The admin page is again at `http://localhost:3000/`.

### Run it as a systemd service (Linux)

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

## Deploying together with DS_WebAgent

If you want the DeepSeek web channel, the repository ships `docker-compose.dswebagent.yaml`, which brings up both services:

```bash
docker compose -f docker-compose.dswebagent.yaml up -d --build
```

Ports, the first login and channel configuration are covered in [DS_WebAgent](/en/DSWebAgent).

## Reverse proxy / HTTPS

The service itself speaks plain HTTP; put Nginx or Caddy in front of it:

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
        proxy_buffering off;          # streaming responses need this
        proxy_read_timeout 1800s;     # web-edition replies can be slow
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

> `proxy_buffering off` / `flush_interval -1` is essential: without it, `stream: true` SSE responses are buffered whole and the client sees no incremental output.

## Security notes

- Change `router.token` (the admin password) right after deploying, and regenerate your API keys;
- Expose only `/v1/chat/completions` to the clients that need it; protect the admin API `/api/*` with an IP allowlist or Basic Auth;
- Do not run the container with `--network host`;
- Back up the `data/` directory regularly.

## Upgrading

```bash
# Docker
docker compose pull && docker compose up -d

# Source
git pull && cd web && npm install && npm run build && cd .. && pip install -r requirements.txt
# then restart the process / service
```

Database migrations are automatic; just keep `data/` mounted.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| Port 3000 serves a blank page | The frontend was not built (manual installs need `web/dist`). Run `cd web && npm run build`, or use the Docker image |
| Admin page says the password is wrong | The password is `router.token` in `data/config.yaml`; a fresh install generates `12345678` |
| A channel is always unavailable | Click the channel's **Test** button to see the error; an adapter that cannot reach its site (missing cookie, not logged in) gets disabled by the watchdog |
| Streaming returns everything at once | Your reverse proxy buffers; see the Nginx / Caddy config above |
| Configuration is gone after a restart | `data/` was not mounted to the host |
