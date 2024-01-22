# 配置文件

配置文件位于`data/config.yaml`

```yaml
# 每个适配器的全局配置
adapters:
  acheong08_ChatGPT:
    # 是否自动忽略重复的响应 see: https://github.com/RockChinQ/free-one-api/issues/75
    auto_ignore_duplicated: false
    # acheong08/ChatGPT 适配器的反向代理地址
    # 默认的公共反代可能不稳定，建议自行搭建，若下方项目均不可用，请自行寻找其他反代:
    # https://github.com/flyingpot/chatgpt-proxy （推荐）
    # https://github.com/acheong08/ChatGPT-Proxy-V4 （不可用）
    reverse_proxy: https://chatproxy.rockchin.top/api/
database:
  # SQLite 数据库文件路径
  path: ./data/free_one_api.db
  type: sqlite
logging:
  debug: false  # 是否开启调试日志
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