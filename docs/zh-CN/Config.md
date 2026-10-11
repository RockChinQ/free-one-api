# 配置文件

配置文件位于 `data/config.yaml`（首次启动自动生成；容器内是 `/app/data/config.yaml`）。
改完重启进程生效。

```yaml
# 每个适配器的全局配置（可作为渠道 Config 的默认值）
adapters:
  acheong08_ChatGPT:
    # 是否自动忽略重复的响应
    auto_ignore_duplicated: false
    # 反向代理地址；默认公共反代压力大，建议自建
    reverse_proxy: https://chatproxy.rockchin.top/api/

  # DS_WebAgent 转发适配器的默认值
  ds-webagent/deepseek-web:
    base_url: http://ds-webagent:9178
    api_token: local-dev-token
    default_model: ds-web-search
    timeout_secs: 900

database:
  # SQLite 数据库文件路径（渠道、Key、日志都在这里）
  path: ./data/free_one_api.db
  type: sqlite

logging:
  debug: false          # 打开后输出调试日志

# 随机广告：会按概率追加到每个响应末尾
random_ad:
  enabled: false
  rate: 0.05
  ad_list:
  - ' (This response is sponsored by Free One API. Consider star the project on GitHub:
    https://github.com/RockChinQ/free-one-api )'

router:
  port: 3000            # 后端监听端口（管理页与 API 同端口）
  token: '12345678'     # 管理页登录密码

watchdog:
  heartbeat:
    fail_limit: 3       # 连续失败几次后自动禁用渠道
    interval: 1800      # 心跳间隔（秒）
    timeout: 300        # 单渠道心跳超时（秒）

web:
  frontend_path: ./web/dist/   # 前端静态文件目录
```

## 各项说明

### adapters

每个适配器的全局配置。渠道自己的 `Config` 优先级更高：渠道里填了就用渠道的，没填就落到这里，再没有就用适配器内置默认值。

### database

只支持 `sqlite`。`path` 指向的目录必须可写且**要持久化**，否则重启后渠道与 Key 全丢。

### router.token

管理页登录密码。生产环境务必改掉默认值，并且不要用弱口令——管理页能读到所有 API Key 与渠道凭据。

### watchdog.heartbeat

后台定时对每个启用渠道调用适配器的 `test()`：

- `interval`：多久跑一次；
- `timeout`：单次超时；
- `fail_limit`：连续失败达到该次数即自动禁用该渠道（禁用后可在管理页手动启用）。

> DS_WebAgent 未登录 DeepSeek 时 `/health` 返回 `ready: false`，会被判定为失败——先登录再启用渠道。

### random_ad

按 `rate` 概率在响应末尾追加 `ad_list` 里随机一条文案。不想要就把 `enabled` 设为 `false`。

### web.frontend_path

前端构建产物目录。手动部署时需要在 `web/` 里 `npm run build`；用仓库的 Dockerfile 则镜像内已构建好。

## 环境变量

Free One API 自身没有环境变量覆盖配置的机制，配置只从 `data/config.yaml` 读。
与 DS_WebAgent 相关的环境变量属于那个容器（如 `DSWA_API_TOKEN`），见 DS_WebAgent 接入。
