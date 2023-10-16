# 配置文件

配置文件位于`data/config.yaml`

```yaml
database:
  # SQLite 数据库文件路径
  path: ./data/free_one_api.db
  type: sqlite
logging:
  debug: false  # 是否开启调试日志
misc:
  # acheong08/ChatGPT 适配器的反向代理路径
  # 默认的公共反代可能不稳定，建议自行搭建:
  # https://github.com/acheong08/ChatGPT-Proxy-V4
  chatgpt_api_base: https://chatproxy.rockchin.top/api/
# 随机广告
# 会随机追加到每个响应的末尾
random_ad:
  # 广告列表
  ad_list:
  - ' (This response is sponsored by Free One API. Consider star the project on GitHub:
    https://github.com/RockChinQ/free-one-api )'
  # 是否开启随机广告
  enabled: false
  # 广告出现概率 (0-1)
  rate: 0.05
router:
  # 后端监听端口
  port: 3000
  # 管理页登录密码
  token: '12345678'
watchdog:
  heartbeat:
    # 自动停用渠道前的心跳失败次数
    fail_limit: 3
    # 心跳检测间隔（秒）
    interval: 1800
    # 单个渠道心跳检测超时时间（秒）
    timeout: 300
web:
  # 前端页面路径
  frontend_path: ./web/dist/
```