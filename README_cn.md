<div align="center">

# free-one-api

[中文文档](README_cn.md) | [English](README.md)

[![GitHub release (latest by date)](https://img.shields.io/github/v/release/RockChinQ/free-one-api)](https://github.com/RockChinQ/free-one-api/releases/latest)
<a href="https://hub.docker.com/repository/docker/rockchin/free-one-api">
    <img src="https://img.shields.io/docker/pulls/rockchin/free-one-api?color=blue" alt="docker pull">
  </a>
![Wakapi Count](https://wakapi.dev/api/badge/RockChinQ/interval:any/project:free-one-api)

<img width="500" alt="image" src="assets/feature.png">

</div>
<hr>

将逆向工程的大语言模型库转换为标准的 OpenAI GPT API。  
通过使用 free-one-api，您可以轻松地将逆向工程的 LLM 库（例如 [acheong08/ChatGPT](https://github.com/acheong08/ChatGPT) ） 转换为标准的 OpenAI GPT API。  
因此，其他支持 OpenAI GPT API 的应用程序可以直接使用逆向工程的 LLM 库。

## 功能点

- 支持自动负载均衡。
- 支持 Web UI。
- 支持流模式。

### 支持的 LLM 库

- [acheong08/ChatGPT](https://github.com/acheong08/ChatGPT) - ChatGPT 网页版逆向工程
    - gpt-3.5-turbo
    - gpt-4

### 支持的 API 路径

- `/v1/chat/completions`

欢迎提交 issue 或 pull request 来添加更多的 LLM 库和 API 路径支持。

## 部署

### Docker (推荐)

```bash
docker run -d -p 3000:3000 --name free-one-api rockchin/free-one-api -v ./data:/app/data
```

你可以在 `http://localhost:3000/` 打开管理页面。

### 手动

```bash
git clone https://github.com/RockChinQ/free-one-api.git
cd free-one-api

cd web && npm install && npm run build && cd ..

pip install -r requirements.txt
python main.py
```

你可以在 `http://localhost:3000/` 打开管理页面。

## Usage

1. 创建一个 channel，按照说明填写配置，然后创建一个新的 key。

![add_channel](assets/add_channel.png)

2. 将 url (e.g. http://localhost:3000/v1 ) 设置为 OpenAI 的 api_base ，将生成的 key 设置为 OpenAI api key。
3. 现在你可以使用 OpenAI API 来访问逆向工程的 LLM 库了。

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

## 快速体验

### Demo

可以登录并修改通道和key数据，每30分钟重置(xx:00/xx:30).

地址：https://foa-demo.rockchin.top  
密码：12345678  

### 测试通道

仅可使用通道，不可登录：

api_base: https://foa.rockchin.top/v1  
api_key: sk-foaumWEd2Jdfb9wrDSEqE5zEJo81XKd0v76yPsgsTWQgRPpe  
model: gpt-3.5-turbo  
