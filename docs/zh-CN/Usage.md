# 使用

整条链路只有三样东西：渠道（Channel） 提供能力，API Key 授权访问，客户端用 OpenAI 协议调用。


客户端 ->(Bearer key)-> free-one-api:3000 ->(选渠道/负载均衡)-> 适配器 -> 上游 LLM


## 1. 创建渠道

打开 http://localhost:3000/ ，用管理密码登录（默认 12345678，写在 data/config.yaml 的 router.token）。

进「渠道」页 → 「添加渠道」：

| 字段 | 说明 |
|---|---|
| 名称 | 便于区分，例如 deepseek-web |
| 适配器 | 选一个逆向工程库，例如 ds-webagent/deepseek-web；见「适配器」页 |
| Config | 该适配器的 JSON 配置，随适配器不同 |
| 模型映射 | 可选，把客户端模型名映射成上游名字，例如 gpt-4o 映射到 gpt-3.5-turbo |

保存后点该渠道的「测试」。测试只验证凭据与连通性，不代表真能对话。

> 渠道被心跳连续判失败 fail_limit 次后会禁用，需要手动重新启用。

## 2. 创建 API Key

「API Key」页 → 新建，得到 sk- 开头的 key。只在创建时能看全，记得存下来。

## 3. 客户端接入

服务地址 http://localhost:3000/v1，协议是 OpenAI 的 /v1/chat/completions。

Python（新版 openai SDK）：

```python
from openai import OpenAI
```

client = OpenAI(base_url="http://localhost:3000/v1", api_key="sk-你的key")

resp = client.chat.completions.create(
    model="gpt-3.5-turbo",
    messages=[{"role": "user", "content": "hello, how are you?"}],
)
print(resp.choices[0].message.content)


流式：

```python
stream = client.chat.completions.create(
    model="gpt-3.5-turbo",
    messages=[{"role": "user", "content": "讲个笑话"}],
    stream=True,
)
for chunk in stream:
    delta = chunk.choices[0].delta.content
    if delta:
        print(delta, end="", flush=True)
```


常见客户端：Cherry Studio / NextChat / LobeChat 里把 API 地址填 http://localhost:3000/v1，Key 填 sk- 开头的 key。

模型名填什么取决于渠道里的适配器：多数适配器只认 gpt-3.5-turbo / gpt-4；用 ds-webagent/deepseek-web 时填 ds-web-search / ds-web-think / ds-web-search-think。

## 4. 工具调用

工具由调用方声明、调用方执行，与 OpenAI 官方一致：

1. 请求里带 tools（可选 tool_choice）；
2. 模型要调工具时返回 finish_reason: tool_calls，message.content 为 null，调用在 message.tool_calls；
3. 你执行工具，把结果以 role=tool + tool_call_id 追加到 messages，再请求一次；
4. 模型基于结果继续，最终返回 finish_reason: stop 与 content。

Python 例子：

```python
tools = [{
    "type": "function",
    "function": {
        "name": "get_weather",
        "description": "查询城市天气",
        "parameters": {
            "type": "object",
            "properties": {"city": {"type": "string"}},
            "required": ["city"],
        },
    },
}]
```

messages = [{"role": "user", "content": "上海现在天气怎么样？"}]

first = client.chat.completions.create(model="gpt-3.5-turbo", messages=messages, tools=tools)
call = first.choices[0].message.tool_calls[0]
messages.append(first.choices[0].message)
messages.append({"role": "tool", "tool_call_id": call.id, "content": "22 度，晴"})

final = client.chat.completions.create(model="gpt-3.5-turbo", messages=messages, tools=tools)
print(final.choices[0].message.content)


> 只有声明了 function_call_supported 的适配器才走这条链；DS_WebAgent 一次只承载一个工具调用。

## 5. 查看用量与日志

- 管理页「日志」标签：每次请求的渠道、耗时、成功或失败；
- 终端 docker compose logs -f free-one-api；
- 数据在 data/free_one_api.db（SQLite）。

## 6. 常见操作

```bash
docker compose stop                      # 停服
docker compose logs -f --tail=100        # 看日志
cp -r data data.bak                      # 备份
```


改管理密码：编辑 data/config.yaml 的 router.token，重启进程生效。
