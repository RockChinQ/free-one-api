# Usage

The whole chain is three things: a channel provides the capability, an API key authorizes access, and clients talk OpenAI to the service.


Client -> (Bearer key) -> free-one-api:3000 -> (channel select / load balance) -> adapter -> upstream LLM


## 1. Create a channel

Open http://localhost:3000/ and log in with the admin password (default 12345678, stored as router.token in data/config.yaml).

Go to Channels -> Add channel:

| Field | Meaning |
|---|---|
| Name | Any label, e.g. deepseek-web |
| Adapter | Pick a reverse-engineering library, e.g. ds-webagent/deepseek-web; see the Adapters page |
| Config | Adapter-specific JSON config |
| Model mapping | Optional; maps a client model name onto an upstream one, e.g. gpt-4o -> gpt-3.5-turbo |

Save, then click the channel's Test button. The test only verifies credentials and connectivity, not a real conversation.

> A channel that keeps failing the heartbeat fail_limit times is disabled automatically and must be re-enabled by hand.

## 2. Create an API key

On the API Key page create a key; it starts with sk-. The full value is shown only once, so store it.

## 3. Connect a client

Base URL is http://localhost:3000/v1 and the protocol is OpenAI's /v1/chat/completions.

Python (modern openai SDK):

```python
from openai import OpenAI
```

client = OpenAI(base_url="http://localhost:3000/v1", api_key="sk-your-key")

resp = client.chat.completions.create(
    model="gpt-3.5-turbo",
    messages=[{"role": "user", "content": "hello, how are you?"}],
)
print(resp.choices[0].message.content)


Streaming:

```python
stream = client.chat.completions.create(
    model="gpt-3.5-turbo",
    messages=[{"role": "user", "content": "tell me a joke"}],
    stream=True,
)
for chunk in stream:
    delta = chunk.choices[0].delta.content
    if delta:
        print(delta, end="", flush=True)
```


Popular clients (Cherry Studio / NextChat / LobeChat): set the API base to http://localhost:3000/v1 and the key to your sk- value.

The model name depends on the adapter: most accept gpt-3.5-turbo / gpt-4; with ds-webagent/deepseek-web use ds-web-search, ds-web-think or ds-web-search-think.

## 4. Tool calling

Tools are declared and executed by the caller, exactly like OpenAI:

1. Send tools (optionally tool_choice) in the request;
2. When the model wants a tool it returns finish_reason: tool_calls, message.content is null and the call is in message.tool_calls;
3. Execute the tool, append the result as role=tool + tool_call_id to messages, and request again;
4. The model continues from the tool result and finally returns finish_reason: stop with content.

Python example:

```python
tools = [{
    "type": "function",
    "function": {
        "name": "get_weather",
        "description": "Look up the weather of a city",
        "parameters": {
            "type": "object",
            "properties": {"city": {"type": "string"}},
            "required": ["city"],
        },
    },
}]
```

messages = [{"role": "user", "content": "What is the weather in Shanghai?"}]

first = client.chat.completions.create(model="gpt-3.5-turbo", messages=messages, tools=tools)
call = first.choices[0].message.tool_calls[0]
messages.append(first.choices[0].message)
messages.append({"role": "tool", "tool_call_id": call.id, "content": "22C, clear"})

final = client.chat.completions.create(model="gpt-3.5-turbo", messages=messages, tools=tools)
print(final.choices[0].message.content)


> Only adapters that report function_call_supported follow this path; DS_WebAgent carries one tool call per response.

## 5. Usage logs

The admin Logs tab lists every request with its channel, latency and success/failure. On the command line:

```bash
docker compose logs -f free-one-api
```


Data lives in data/free_one_api.db (SQLite).

## 6. Common operations

```bash
docker compose stop                      # stop the service
docker compose logs -f --tail=100        # tail logs
cp -r data data.bak                      # back up
```


To change the admin password, edit router.token in data/config.yaml and restart.
