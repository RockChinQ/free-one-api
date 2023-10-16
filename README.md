
[中文](README.md) | [English](README_en.md)

<div align="center">

<img width="150" alt="image" src="web/src/assets/logo.png">

# free-one-api

通过标准的 OpenAI API 格式访问所有的 LLM 逆向工程库

![Static Badge](https://img.shields.io/badge/Free-100%25-green)
[![GitHub release (latest by date)](https://img.shields.io/github/v/release/RockChinQ/free-one-api)](https://github.com/RockChinQ/free-one-api/releases/latest)
<a href="https://hub.docker.com/repository/docker/rockchin/free-one-api">
    <img src="https://img.shields.io/docker/pulls/rockchin/free-one-api?color=green" alt="docker pull">
  </a>
![Wakapi Count](https://wakapi.dev/api/badge/RockChinQ/interval:any/project:free-one-api)

</div>

> 欲通过 OpenAI 标准 API 访问各个 LLM 的**官方接口(付费)**，可以使用 [songquanpeng/one-api](https://github.com/songquanpeng/one-api)，`free-one-api` 亦可与 `one-api` 项目搭配使用。

## 功能点

- 支持自动负载均衡。
- 支持 Web UI。
- 支持流模式。
- 支持多个 LLM 逆向库。
- 心跳检测机制、自动禁用不可用的渠道。
- 运行日志记录。

<details>
<summary>截图展示</summary>

**渠道页面:**

<img width="400" alt="image" src="assets/channels.png">

**添加渠道:**

<img width="400" alt="image" src="assets/add_channel.png">

**Curl:**

<img width="400" alt="image" src="assets/feature.png">

</details>

### 支持的 LLM 库

|Adapter|Multi Round|Stream|Function Call|Status|Comment|
|---|---|---|---|---|---|
|[acheong08/ChatGPT](https://github.com/acheong08/ChatGPT)|✅|✅|❌|✅|ChatGPT 网页版|
|[KoushikNavuluri/Claude-API](https://github.com/KoushikNavuluri/Claude-API)|✅|❌|❌|✅|Claude 网页版|
|[dsdanielpark/Bard-API](https://github.com/dsdanielpark/Bard-API)|✅|❌|❌|✅|Google Bard 网页版|
|[xtekky/gpt4free](https://github.com/xtekky/gpt4free)|✅|✅|❌|✅|gpt4free 接入多个平台的破解|
|[Soulter/hugging-chat-api](https://github.com/Soulter/hugging-chat-api)|✅|✅|❌|✅|huggingface的对话模型|
|[xw5xr6/revTongYi](https://github.com/xw5xr6/revTongYi)|✅|✅|❌|✅|阿里云通义千问网页版|

### 支持的 API 路径

- `/v1/chat/completions`

欢迎提交 issue 或 pull request 来添加更多的 LLM 库和 API 路径支持。

## 快速体验

### Demo

可以登录并修改通道和key数据，每30分钟重置(xx:00/xx:30).

地址：https://foa-demo.rockchin.top  
密码：12345678  

### 测试通道

仅可使用通道，不可登录：

api_base: https://foa.rockchin.top/v1  
api_key: sk-foaDfZxzvfrwfqkBDJEMq7C0rdXkhOjXx4aM23pH42tv8SJ4  
model: gpt-3.5-turbo  

## 负载均衡性能

启用4个通道，客户端16个线程的请求时间甘特图，请求提问“write a quick sort in Java”：  
(通道标记为 `Channel ID <id> <request count>`, X 轴为时间秒数, 每个色块为一次请求)

<img width="750" alt="image" src="assets/load_balance.png">
