# FAQ

## Deployment and running

### What is the default admin password, and how do I change it?

12345678, stored as router.token in data/config.yaml. Change it and restart the process.

### Port 3000 shows a blank page?

The frontend was not built. Manual installs must run npm install && npm run build inside web/; the output is web/dist, which the server serves. The repository Dockerfile builds it inside the image, so it never hits this.

### Channels and keys are gone after a restart?

The data/ directory was not mounted to the host. Inside the container the path is always /app/data, holding free_one_api.db and config.yaml.

```bash
docker run -v ~/free-one-api/data:/app/data ...
```


### Port 3000 is already in use?

Change router.port in data/config.yaml, or change the compose port mapping (e.g. "8080:3000"; the container port stays).

### How do I upgrade?

```bash
# Docker image
docker compose pull && docker compose up -d
```

# Source install
git pull && cd web && npm install && npm run build && cd ..
docker compose -f docker-compose.dswebagent.yaml up -d --build   # if you use the bundled compose


Database migrations are automatic; keep data/ mounted.

## Channels and requests

### The channel test passes but real requests fail?

The test only calls the adapter's own test() (for DS_WebAgent, one /health probe); it does not prove a conversation works. Check docker compose logs -f free-one-api for the upstream error.

### No suitable channel found?

Channel selection has hard filters: the channel must be enabled, its adapter's supported_path() must equal the request path (/v1/chat/completions), and the model name must be among the channel's supported models. Check:

- whether the watchdog disabled the channel (status not green);
- whether the requested model is one the channel accepts.

### The watchdog disabled my channel?

After watchdog.heartbeat.fail_limit (default 3) consecutive failures a channel is disabled; the interval defaults to 1800s. While DS_WebAgent is not logged in, /health returns ready: false and counts as a failure — log in, then re-enable the channel.

### stream: true does not stream?

Two possibilities:

1. The reverse proxy buffers. Add proxy_buffering off; in Nginx or flush_interval -1 in Caddy;
2. Some adapters do not support streaming (see the Stream column on the Adapters page); they yield the whole reply at once.

### Why is there an ad at the end of a reply?

random_ad in data/config.yaml is off by default. If enabled, rate is the probability and ad_list the texts.

### Does it support concurrency?

The framework does multi-channel routing and load balancing, but per-channel concurrency depends on the adapter. DS_WebAgent is serial internally (max_concurrent_requests = 1), so concurrent requests queue.

## DS_WebAgent

### I cannot see the Chromium inside the container?

It runs on the Xvfb virtual display and is exposed through noVNC: http://localhost:16080/vnc.html. You must log in to DeepSeek from there the first time.

### I logged in once — do I need to log in again after a restart?

No. The login state lives in ./ds_webagent-data/browser-profile (/data/browser-profile in the container); as long as the volume is not deleted it persists.

### /health keeps saying login_required?

You are not logged in yet. Open noVNC, log in to DeepSeek, and /health turns ready.

### address already in use for 9178?

Host port 9178 is taken. Change the host port of ds-webagent in the compose file (e.g. "19178:9178"). Leave the container port alone — free-one-api reaches it via ds-webagent:9178.

### Channel test returns 401?

The api_token does not match DS_WebAgent's [server].api_token.

### Channel test says it cannot connect?

Wrong base_url. Inside the same compose network it must be http://ds-webagent:9178, not 127.0.0.1 (that is the container itself).

### Do I need to restart after editing the channel Config?

No. This adapter re-reads its config on every request, so saving is enough.

### The logs keep printing chromiumoxide WS Invalid message?

Chromium's CDP messages are newer than the chromiumoxide library expects; harmless noise.

### How does tool calling work?

Send tools; when the model wants a tool it returns finish_reason: "tool_calls", you execute it, then append a role=tool result to messages and request again. Full examples are on the DS_WebAgent page.

### Can several tools run in parallel in one response?

No. DS_WebAgent carries one tool call per response; use multiple rounds for multiple tools.

## Compliance

This project only drives the normal web UI of each site: no private-API reverse engineering, no captcha / risk-control bypass, no browser fingerprint spoofing. Keep usage low-frequency and personal.
