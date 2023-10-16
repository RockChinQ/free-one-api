# Usage

1. Create a channel and fill in the name.

![add_channel](assets/add_channel.png)

2. Select the reverse engineering library adapter used by this channel and fill in the configuration.

> Please refer to the [Adapters](/en/Adapters.md) document.

3. Create a new key in the API Key column.

4. Set the url (e.g. http://localhost:3000/v1 ) as OpenAI's api_base and the generated key as OpenAI api key.
5. Now you can use the OpenAI API to access the reverse engineering LLM library.

## Testing

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
