# DS_WebAgent (DeepSeek web edition)

ds_web_agent/ is a Rust bridge bundled with this repository: it launches a real, headed Chromium, drives chat.deepseek.com through its normal UI, and exposes an OpenAI-compatible API.

The upside: no cookie, no session token, no private API. The cost: you have to log in once in that browser.

## Architecture


OpenAI client
   |  POST /v1/chat/completions
   v
free-one-api  (Python, port 3000)
   |  channel adapter ds-webagent/deepseek-web  -- forwarded as-is
   v
DS_WebAgent  (Rust, container port 9178)
   |  drives a real Chromium
   v
chat.deepseek.com web edition


Both processes run in containers and reach each other by service name (http://ds-webagent:9178), so no host ports are needed between them.

## One-command deployment

docker-compose.dswebagent.yaml in the repository root:

```yaml
services:
  ds-webagent:
    build:
      context: ./ds_web_agent
    image: ds-webagent:local
    container_name: ds-webagent
    restart: always
    shm_size: "1gb"                 # Chromium needs a bigger /dev/shm
    environment:
      DSWA_API_TOKEN: local-dev-token
    volumes:
      - ./ds_webagent-data:/data    # login state, logs and stats live here
    ports:
      - "19178:9178"                # host 19178 -> container 9178
      - "16080:6080"                # noVNC desktop
```

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


```bash
docker compose -f docker-compose.dswebagent.yaml up -d --build
```


The first build compiles the Rust dependencies and takes roughly 5–8 minutes; after that, Python changes only rebuild free-one-api.

> The host ports are 19178 / 16080 to avoid a busy local 9178. If nothing occupies them on your machine, change ports back to "9178:9178" and "6080:6080".

## First login to DeepSeek

Chromium inside the container is headed, but it runs on the Xvfb virtual display; noVNC gives you a window into it:

1. Open http://localhost:16080/vnc.html;
2. Click Connect; Chromium should already be on chat.deepseek.com/sign_in;
3. Log in normally (password or QR code);
4. Check http://localhost:19178/health, which should now report:

```json
{ "status": "ready", "ready": true, "detail": "logged in, /v1/chat/completions is available" }
```


The login state is stored in ./ds_webagent-data/browser-profile, so restarting the container does not require logging in again.

> Do not want noVNC? Drop 16080:6080 from the compose file, or set DSWA_ENABLE_VNC=false. But then you cannot log in by hand, so keep it for the first login.

## Adding the channel

1. Open http://localhost:3000/ and log in with the admin password (default 12345678);
2. Channels -> Add channel;
3. Adapter: ds-webagent/deepseek-web;
4. Config:

```json
{
  "base_url": "http://ds-webagent:9178",
  "api_token": "local-dev-token",
  "default_model": "ds-web-search",
  "timeout_secs": 900
}
```


5. Save, then click Test on that channel (it probes /health) and wait for it to turn green;
6. Create an API key on the API Key page and point your client at http://localhost:3000/v1.

### Config fields

| Field | Required | Default | Meaning |
|---|---|---|---|
| base_url | no | http://127.0.0.1:9178 | DS_WebAgent address. In the same compose network use http://ds-webagent:9178; running natively use http://127.0.0.1:9178 |
| api_token | no | local-dev-token | Must match DS_WebAgent's [server].api_token, otherwise 401 |
| default_model | no | ds-web-search | Fallback variant when the client sends an unknown model name |
| timeout_secs | no | 900 | Per-request timeout. Web-edition replies are slow, do not set it too low |

### Hot config changes

The adapter re-reads its config on every request: edit the channel Config in the admin page and save; the next request immediately uses the new address / token, no container restart needed.

## Model names

DS_WebAgent knows exactly three model IDs, mapping to the combinations of the web page's “web search / deep think” toggles:

| model id | Web search | Deep think | Notes |
|---|---|---|---|
| ds-web-search | yes | no | search without thinking |
| ds-web-think | no | yes | thinking without search |
| ds-web-search-think | yes | yes | search and think |

An unknown model name (e.g. gpt-4o) does not error; it falls back to default_model.

## Tool calling

Tools are declared and executed by the caller, matching OpenAI:

1. Send tools (optionally tool_choice) in the request; the channel forwards it as-is;
2. When the model decides to call a tool it returns finish_reason: "tool_calls", message.content is null and the call is in message.tool_calls;
3. Execute the tool, append {"role":"tool","tool_call_id":"...","content":"..."} to messages and request again;
4. The model continues from the tool result and finally returns finish_reason: "stop" with content.

First request:

```bash
curl http://localhost:3000/v1/chat/completions \
  -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_KEY" \
  -d '{
    "model": "ds-web-search",
    "messages": [{"role": "user", "content": "What is the weather in Shanghai?"}],
    "tools": [{
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Look up the weather of a city",
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


When the model wants a tool:

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
        "function": {"name": "get_weather", "arguments": "{\"city\": \"Shanghai\"}"}
      }]
    },
    "finish_reason": "tool_calls"
  }]
}
```


Second round carries the tool result back:

```bash
curl http://localhost:3000/v1/chat/completions \
  -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_KEY" \
  -d '{
    "model": "ds-web-search",
    "messages": [
      {"role": "user", "content": "What is the weather in Shanghai?"},
      {"role": "assistant", "content": null, "tool_calls": [{
        "id": "call_xxx", "type": "function",
        "function": {"name": "get_weather", "arguments": "{\"city\": \"Shanghai\"}"}
      }]},
      {"role": "tool", "tool_call_id": "call_xxx", "name": "get_weather", "content": "{\"temp\": 22, \"desc\": \"clear\"}"}
    ],
    "tools": [{"type": "function", "function": {"name": "get_weather", "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}}}]
  }'
```


With stream: true, tool_calls appear in choices[0].delta.tool_calls of the SSE chunks and finish_reason is still "tool_calls".

### Limits

- One response carries a single tool call (a DS_WebAgent internal constraint); multiple parallel tools need multiple rounds;
- Arguments are parsed as JSON and returned with arguments as a JSON string (same shape as OpenAI).

## Port conflicts

If host port 9178 is already taken (for example you run a native DS_WebAgent too), compose reports:


failed to bind host port 0.0.0.0:9178/tcp: address already in use


Change the host port in docker-compose.dswebagent.yaml (leave the container port alone; free-one-api reaches it by service name):

```yaml
    ports:
      - "19178:9178"
      - "16080:6080"
```


## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| /health keeps returning login_required | Not logged in yet. Open noVNC and log in, then reload |
| Channel test returns 401 | api_token does not match DS_WebAgent's [server].api_token |
| Channel test says it cannot connect | Wrong base_url. Inside the compose network use http://ds-webagent:9178, not 127.0.0.1 |
| Requests are slow / queued | DS_WebAgent is serial internally (max_concurrent_requests = 1); concurrency queues, which is expected |
| Logs keep printing chromiumoxide WS Invalid message | Chromium is newer than the library expects; harmless noise |
| Container is up but Chromium crashes | /dev/shm too small. Keep shm_size: "1gb"; the config already passes --disable-dev-shm-usage |
| Test passes but real requests fail | The test only probes /health. Confirm the DeepSeek page itself can chat (still logged in, no captcha) |

Logs:

```bash
docker logs -f ds-webagent
docker logs -f free-one-api
tail -f ./ds_webagent-data/logs/deeptoolbridge.log
```


## Compliance

- Only the normal web UI is driven; no private DeepSeek API is called;
- No captcha / risk-control / Cloudflare bypass, no browser fingerprint spoofing;
- Low frequency, personal use only.
