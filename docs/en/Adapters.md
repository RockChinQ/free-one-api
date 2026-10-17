# Adapters

Free One API currently supports multiple LLM reverse engineering libraries, each channel supports a corresponding adapter, the adapter is responsible for converting the client's request into a reverse engineering library request, and converting the reverse engineering library's response into the client's response.

## acheong08/ChatGPT

ChatGPT official website reverse engineering library

### Configuration

1. Select `acheong08/ChatGPT` as `Adapter`

![select adapter](assets/select_adapter.png)

2. Go to `chat.openai.com` and log in to your account

3. Access `https://chat.openai.com/api/auth/session` directly in the browser, copy the `access_token` obtained

![Alt text](assets/get_actoken.png)

4. Enter in the `Config` column

```json
{
  "access_token": "your access token"
}
```

5. Save to test

### Reverse proxy

ChatGPT needs to use a reverse proxy to bypass Cloudflare's restrictions. The Free One API project defaults to the proxy address provided by the developer `https://chatproxy.rockchin.top/api/`, but the pressure is very high. It is strongly recommended to build a reverse proxy by yourself.

* You can build it using the following projects, if they are all unavailable, please find other reverse proxies by yourself:
  - https://github.com/flyingpot/chatgpt-proxy (Recommended)
  - https://github.com/acheong08/ChatGPT-Proxy-V4 (Unavailable)

Modify `adapters.acheong08_ChatGPT.reverse_proxy` to your reverse proxy address in `data/config.yaml`.
You can also directly enter in the `Config` column when creating the `acheong08/ChatGPT` adapter

```json
{
  "reverse_proxy": "your reverse proxy address"
}
```

Set the reverse proxy address used by this adapter.

> **WARNING**  
> The current reverse proxy may have [the situation of repeating the previous text](https://github.com/RockChinQ/free-one-api/issues/75)(CN), `free-one-api` will automatically delete duplicate content. If unexpected situations occur, please set `adapters.acheong08_ChatGPT.auto_ignore_duplicated` to `false` to disable this feature.

## KoushikNavuluri/Claude-API

Anthropic Claude official website reverse engineering library

### Configuration

1. Select `KoushikNavuluri/Claude-API` as `Adapter`

2. Log in to `claude.ai`, open `F12`, select the `Network` column, find any request, and copy the `Cookie` string in the request header

![claude_get_cookie](assets/claude_cookie.png)

3. Enter in the `Config` column

```json
{
  "cookie": "your cookie"
}
```

## xtekky/gpt4free

xtekky/gpt4free integrates multiple LLM reverse engineering libraries of multiple platforms

### Configuration

1. Select `xtekky/gpt4free` as `Adapter`

2. No authentication required, just save

## Soulter/hugging-chat-api

huggingface.co/chat official website reverse engineering library

### Configuration

1. Register `HuggingFace` account

2. Select `Soulter/hugging-chat-api` as `Adapter`

3. Enter in the `Config` column

```json
{
  "email": "HuggingFace Email",
  "passwd": "HuggingFace Password"
}
```

## xw5xr6/revTongYi

Aliyun TongYi QianWen official website reverse engineering library

### Configuration

1. Select `xw5xr6/revTongYi` as `Adapter`

2. Go to <https://qianwen.aliyun.com/> and log in to your account

3. Refer to the configuration method of Claude above to obtain the `Cookie` string

4. Enter in the `Config` column

```json
{
  "cookie": "通义千问cookie"
}
```

## Zai-Kun/reverse-engineered-chatgpt

ChatGPT official website reverse engineering library, this is newer. acheong08/ChatGPT is no longer maintained.

### Configuration

1. Select `Zai-Kun/reverse-engineered-chatgpt` as `Adapter`

2. Go to`chat.openai.com` and log in to your account

3. Take the value of `__Secure-next-auth.session-token` from Cookies as `session_token`

4. Enter in the `Config` column

```json
{
  "session_token": "your session token"
}
```

## ds-webagent/deepseek-web

Expose a locally running DS_WebAgent as a channel. DS_WebAgent is the Rust bridge in ds_web_agent/: it drives a real (headed) browser on chat.deepseek.com and exposes an OpenAI-compatible API, so no cookie or session token is needed — but you must log in to DeepSeek once in that browser.

### Deployment

Start it together with the bundled docker-compose.dswebagent.yaml (recommended):

```bash
docker compose -f docker-compose.dswebagent.yaml up -d --build
```

Then:

1. Open http://localhost:16080/vnc.html and log in to DeepSeek in the Chromium inside the container (the login state persists in ./ds_webagent-data/browser-profile, so a restart does not require logging in again);
2. Open http://localhost:3000/ for this project’s admin page (default password 12345678);
3. Add channel -> Adapter: ds-webagent/deepseek-web -> Save -> Test.

### Configuration

```json
{
  "base_url": "http://ds-webagent:9178",
  "api_token": "local-dev-token",
  "default_model": "ds-web-search",
  "timeout_secs": 900
}
```

| Field | Meaning |
|---|---|
| base_url | DS_WebAgent address. Inside the compose network use http://ds-webagent:9178; running natively use http://127.0.0.1:9178 |
| api_token | Must match DS_WebAgent’s [server].api_token, otherwise 401 |
| default_model | Fallback when the client’s model name is not one of the three below |
| timeout_secs | Per-request timeout (web-edition replies are slow; default 900s) |

### Model names

DS_WebAgent knows exactly three model IDs, mapping to the combinations of the web page’s “web search / deep think” toggles:

| model id | Web search | Deep think |
|---|---|---|
| ds-web-search | on | off |
| ds-web-think | off | on |
| ds-web-search-think | on | on |

An unknown model name (e.g. gpt-4o) falls back to default_model instead of erroring.

### Tool calling (supported)

The tools / tool_choice fields are forwarded to DS_WebAgent as-is; when the model wants a tool this channel returns finish_reason: "tool_calls" and message.tool_calls, and the caller executes the tool:

1. Execute the tool, append {"role": "tool", "tool_call_id": "...", "content": "..."} to messages and request again;
2. The model continues from the tool result and finally returns finish_reason: "stop" with content.

DS_WebAgent carries one tool call per response, so message.tool_calls always has length 1.

### Hot config changes

The adapter re-reads its config on every request: edit the channel Config in the admin page and save; the next request uses it immediately, no container restart needed.

### Known limits

- DS_WebAgent is serial internally (max_concurrent_requests = 1), so concurrent requests queue; while DeepSeek is not logged in, /health returns ready: false and the channel is treated as unavailable — expected.
- The channel’s Test button only probes DS_WebAgent’s /health; it does not send a real conversation.

## aeliusai/aelius-web

Reverse-proxies the web chat at <https://aeliusai.com/chat>. The adapter only makes the same requests the web UI makes; it reuses the site’s own guest session, chats and SSE streaming endpoints, and does not bypass captchas or Cloudflare:

- `POST /api/v1/guest/session`: create a guest identity, no account required;
- `POST /api/v1/chats`: create a web chat;
- `POST /api/v1/chats/{id}/stream`: send one turn, streamed back as SSE;
- `POST /api/v1/chats/{id}/regenerate`: continue a turn that was submitted but never answered.

Guest mode works out of the box; supplying a signed-in cookie gives a larger context window (roughly 6k tokens as a guest and 22k tokens signed in, subject to the site’s actual quota).

### Configuration

1. Open http://localhost:3000/ for the admin page, choose 「Add channel」 -> `Adapter`: `aeliusai/aelius-web`;
2. For guest mode just save. For a signed-in account, enter in the `Config` column:

```json
{
  "session_cookie": "ael_sb_id=...; aelius_did=...; aelius_session=...",
  "timeout_secs": 300
}
```

3. Save, then click 「Test」: the adapter calls `GET /api/v1/user` to check the identity (it does not send a real conversation).

How to get the cookie: sign in at aeliusai.com, open DevTools → Network → any `/api/v1/` request → copy the full `Cookie` request header. The cookie only lives in the channel config (`data/free_one_api.db`, gitignored); never commit it or paste it into docs.

| Field | Meaning |
|---|---|
| session_cookie | Optional. Empty means a guest session (valid 24 hours); a signed-in cookie pins that account |
| tools_enabled | Turn the site’s own web/tool switches on, default `false`; unrelated to client `tools` |
| timeout_secs | Per-reply timeout, default 300s (minimum 30s) |
| max_cached_chats | How many web chats are kept in memory for multi-round reuse, default 8 |
| base_url / user_agent | Usually not needed; only for self-hosted reverse proxies or debugging |

The channel config is read on every request: after editing `Config` in the admin page and saving, the next request uses it — no container restart.

### Model names

The adapter accepts `aelius`, `aelius-web` and `aelius-chat`; `aelius` is the recommended client-side name. The site’s internal model name is never exposed to clients. To use any other name, map it to `aelius` in the channel’s `model_mapping`. `/v1/models` aggregates `model_mapping` keys and each enabled adapter’s `supported_models()`.

### Multi-round conversations

Aelius keeps chat history server-side; the `messages` a client sends are only a replay. The adapter matches on two things:

1. the first `user` message equals the one of the web chat;
2. the preceding `assistant` message in the replayed history is exactly the reply the adapter returned last turn.

When both hold, it reuses the same web chat and sends only the final `user` message; otherwise it starts a new web chat with the rendered full history (`System:` / `User:` / `Assistant:` / `Tool result:` labels joined into one message).

If a previous turn was submitted but never answered, the next request calls the site’s regenerate endpoint instead of sending the same message twice. The chat index is in memory and keeps at most `max_cached_chats` entries.

### Streaming and tools

- **Streaming**: supported. `content` delta frames from the site’s SSE stream become OpenAI streaming chunks, and `done` / `stopped` frames end the turn.
- **Tool calling**: client `tools` / `tool_choice` are not supported; the channel reports `Function Call = ❌`. `tools_enabled` only turns on the site’s own web/tool switches and never produces `tool_calls`.

### Error codes

Failures surface these `code` values, useful for the logs and the admin page:

| code | HTTP | Meaning |
|---|---|---|
| aelius_rate_limited | 429 | Site rate limit (about 12 requests/minute); retry later |
| aelius_pool_full | 429 | No context slot available on the site right now; retry later |
| aelius_context_full | 400 | The current web chat’s context is full; start a new one (or use a signed-in cookie) |
| aelius_auth_expired | 401 | The configured cookie expired; update `session_cookie` |
| aelius_guest_session_failed | 502 | Guest session creation failed |
| aelius_unreachable | 502 | Cannot reach the site (network or self-hosted proxy) |
| aelius_upstream_error | 502 | The site returned a non-2xx status |
| aelius_bad_response | 502 | The create-chat response could not be parsed |
| aelius_stream_broken / aelius_stream_incomplete | 502 | SSE interrupted, or the stream ended without a `done` frame |
| aelius_empty_reply | 502 | Empty reply for this turn |
| aelius_chat_gone | 502 | The web chat is gone and stayed unavailable after rebuilding |
| aelius_attachment_not_supported | 400 | The message carries an image/file or other non-text attachment |
| aelius_empty_messages / aelius_unexpected_last_role | 400 | Nothing sendable in the request, or the last message is not `user` |

### Known limits

- Guest sessions last 24 hours and have a small context; a signed-in account has a larger one;
- The guest session and chat index live in memory and are rebuilt after a free-one-api restart;
- Requests within one channel run serially so a single web chat is never written concurrently;
- Text only; the site’s rate limit is reported as 429;
- The channel’s 「Test」 button only probes `GET /api/v1/user` and never sends a real conversation.
