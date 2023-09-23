<div align="center">

# free-one-api

[中文文档](README.md) | [English](README_en.md)

[![GitHub release (latest by date)](https://img.shields.io/github/v/release/RockChinQ/free-one-api)](https://github.com/RockChinQ/free-one-api/releases/latest)
<a href="https://hub.docker.com/repository/docker/rockchin/free-one-api">
    <img src="https://img.shields.io/docker/pulls/rockchin/free-one-api?color=blue" alt="docker pull">
  </a>
![Wakapi Count](https://wakapi.dev/api/badge/RockChinQ/interval:any/project:free-one-api)

<img width="500" alt="image" src="assets/feature.png">

</div>
<hr>

Turn reverse engineered LLM lib to standard OpenAI GPT API.  
By using free-one-api, you can easily convert reverse engineered LLM libs (e.g. [acheong08/ChatGPT](https://github.com/acheong08/ChatGPT) ) to standard OpenAI GPT API.  
So other application supports OpenAI GPT API can use reverse engineered LLM libs directly.

## Features

- Automatically load balance.
- Web UI.
- Stream mode supported.

### Supported LLM libs

- [acheong08/ChatGPT](https://github.com/acheong08/ChatGPT)
    - gpt-3.5-turbo
    - gpt-4

### Supported API paths

- `/v1/chat/completions`

File a issue or pull request if you want to add more.

## Setup

### Docker (Recommended)

```bash
docker run -d -p 3000:3000 --name free-one-api rockchin/free-one-api -v ./data:/app/data
```

then you can open the admin page at `http://localhost:3000/`.

### Manual

```bash
git clone https://github.com/RockChinQ/free-one-api.git
cd free-one-api

cd web && npm install && npm run build && cd ..

pip install -r requirements.txt
python main.py
```

then you can open the admin page at `http://localhost:3000/`.

## Usage

1. Create channel on the admin page, create a new key.

![add_channel](assets/add_channel.png)

2. Set the url (e.g. http://localhost:3000/v1 ) as OpenAI endpoint, and set the generated key as OpenAI api key.  
3. Then you can use the OpenAI API to access the reverse engineered LLM lib.

```curl
# curl example
curl http://localhost:3000/v1/chat/completions \
  -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -d '{
    "model": "gpt-3.5-turbo",
    "messages": [
      {
        "role": "system",
        "content": "You are a helpful assistant."
      },
      {
        "role": "user",
        "content": "Hello!"
      }
    ],
    "stream": true
  }'
```

```python
# python example
import openai

openai.api_base = "http://localhost:3000/v1"
openai.api_key = "generated key"

response = openai.ChatCompletion.create(
    model="gpt-3.5-turbo",
    messages=[
        {
            "role": "user",
            "content": "hello, how are you?"
        }
    ],
    stream=False,
)

print(response)
```

## Quick Test

### Demo

Allow to login and modify the channel/apikey settings.Reset every 30 minutes(xx:00/xx:30).

Address: https://foa-demo.rockchin.top  
Password: 12345678

### Test channel

Can only use the channel, can't login:

api_base: https://foa.rockchin.top/v1  
api_key: sk-foaumWEd2Jdfb9wrDSEqE5zEJo81XKd0v76yPsgsTWQgRPpe  
model: gpt-3.5-turbo  
