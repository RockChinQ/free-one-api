# Free One API Documentation

> **NOTE**  
> Some of the content in this document is translated by GPT (GitHub Copilot).

## Supported LLM libs

|Adapter|Multi Round|Stream|Function Call|Status|Comment|
|---|---|---|---|---|---|
|[acheong08/ChatGPT](https://github.com/acheong08/ChatGPT)|✅|✅|❌|✅|ChatGPT Web Version|
|[KoushikNavuluri/Claude-API](https://github.com/KoushikNavuluri/Claude-API)|✅|❌|❌|✅|Claude Web Version|
|[dsdanielpark/Bard-API](https://github.com/dsdanielpark/Bard-API)|✅|❌|❌|✅|Google Bard Web Version|
|[xtekky/gpt4free](https://github.com/xtekky/gpt4free)|✅|✅|❌|✅|gpt4free cracked multiple platforms|
|[Soulter/hugging-chat-api](https://github.com/Soulter/hugging-chat-api)|✅|✅|❌|✅|hubbingface chat model|
|[xw5xr6/revTongYi](https://github.com/xw5xr6/revTongYi)|✅|✅|❌|✅|Aliyun TongYi QianWen Web Version|

## Supported API paths

- `/v1/chat/completions`

File a issue or pull request if you want to add more.

## Performance

Gantt chart of request time with 4 channel enabled and 16 threads in client side, querying question "write a quick sort in Java":  
(Channel labelled with `Channel ID <id> <request count>`, X axis is time in seconds)

![Load Balance](assets/load_balance.png)
