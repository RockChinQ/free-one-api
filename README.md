
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

<img width="400" alt="image" src="docs/zh-CN/assets/add_channel.png">

**Curl:**

<img width="400" alt="image" src="assets/feature.png">

</details>

## 文档

部署、配置方式请参考文档：

- GitHub Page: https://rockchinq.github.io/free-one-api
- 自部署文档：https://free-one-api.rockchin.top