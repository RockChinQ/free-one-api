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
