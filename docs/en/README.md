# Free One API — Documentation

One endpoint, one key, one protocol: every reverse-engineered LLM library behind a standard OpenAI API.

Use it freely — the bill has already been paid.

## Where to go

| Page | What is in it |
|---|---|
| Deployment | Docker / Compose / manual / systemd / reverse proxy / upgrades |
| Usage | Create channels and keys, connect clients, tool calling |
| Adapters | Per-adapter configuration |
| DS_WebAgent | DeepSeek web channel: deploy, log in, configure, tool calling |
| Config File | Every field of data/config.yaml |
| FAQ | Troubleshooting quick reference |

## Three-minute quickstart

```bash
# 1. Start the service (Docker image already contains the built frontend)
docker run -d --name free-one-api --restart always \
  -p 3000:3000 -v ~/free-one-api/data:/app/data rockchin/free-one-api

# 2. Open the admin page, create a channel, create an API key
#    http://localhost:3000/    (default password 12345678)

# 3. Use it like OpenAI
curl http://localhost:3000/v1/chat/completions \
  -X POST -H "Content-Type: application/json" \
  -H "Authorization: Bearer <your key>" \
  -d '{"model":"gpt-3.5-turbo","messages":[{"role":"user","content":"Hello!"}]}'
```

Want the DeepSeek web edition (no cookie needed, driven through a real browser)? See [DS_WebAgent](/en/DSWebAgent).

## Supported adapters

| Adapter | Multi Round | Stream | Function Call | Notes |
|---|---|---|---|---|
| acheong08/ChatGPT | ✅ | ✅ | ❌ | ChatGPT web edition |
| KoushikNavuluri/Claude-API | ✅ | ❌ | ❌ | Claude web edition |
| dsdanielpark/Bard-API | ✅ | ❌ | ❌ | Google Bard web edition |
| xtekky/gpt4free | ✅ | ✅ | ❌ | Aggregates many platforms |
| Soulter/hugging-chat-api | ✅ | ✅ | ❌ | HuggingFace chat models |
| xw5xr6/revTongYi | ✅ | ✅ | ❌ | Aliyun TongYi QianWen web edition |
| Zai-Kun/reverse-engineered-chatgpt | ✅ | ✅ | ❌ | ChatGPT web edition (newer) |
| ds-webagent/deepseek-web | ✅ | ✅ | ✅ | DeepSeek web edition driven by the bundled DS_WebAgent (Rust + real browser), no cookie required |
| aeliusai/aelius-web | ✅ | ✅ | ❌ | Reverse-proxies the aeliusai.com/chat web chat; guest session by default, no cookie needed |

## Supported API paths

- `/v1/chat/completions`

Issues and pull requests adding more libraries or paths are welcome.

## Core features

- **Automatic load balancing** — channels are scored and the best one is picked; failures degrade gracefully.
- **Web UI** — channels, keys and logs all manageable from the browser.
- **Streaming** — standard SSE chunks.
- **Tool calling** — `tools` / `tool_choice` are forwarded and `tool_calls` come back per the OpenAI spec (the caller executes the tools).
- **Watchdog** — a channel failing repeatedly is disabled automatically.
- **Run logs** — written to the terminal and the database.

## Load balancing performance

Four channels, a 16-thread client asking “write a quick sort in Java”:
(marker = `Channel ID <id> <request count>`, X axis is seconds, each block is one request)

![Load balancing performance](assets/load_balance.png)

## Community

Questions, reverse-engineering chat, feature requests or contributions — join us:

- QQ group 1: (full)
- QQ group 2: 1091113674
- [Telegram](https://t.me/+qpeq4Ev9FuI0YjVl)
- [Discord](https://discord.gg/W44yRcG5b)

## Compliance

This project and its adapters only drive the normal web UI of each site. They do not reverse private APIs, do not bypass captchas / risk control / Cloudflare, and do not forge browser fingerprints. Keep usage low-frequency and personal.
