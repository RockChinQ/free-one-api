# Configurations

Configuration file is saved at `data/config.yaml`

```yaml
# Global configuration for each adapter
adapters:
  acheong08_ChatGPT:
    # Whether to auto ignore duplicated response. see: https://github.com/RockChinQ/free-one-api/issues/75
    auto_ignore_duplicated: false
    # Reverse proxy address for acheong08/ChatGPT adapter.
    # Default public reverse proxy may be unstable, it is recommended to build your own:
    # https://github.com/flyingpot/chatgpt-proxy (Recommended)
    # https://github.com/acheong08/ChatGPT-Proxy-V4 (Unavailable)
    reverse_proxy: https://chatproxy.rockchin.top/api/
database:
  # SQLite DB file path
  path: ./data/free_one_api.db
  type: sqlite
logging:
  debug: false  # Enable debug log
# Random advertisement, will be appended to the end of each response
random_ad:
  # advertisement list
  ad_list:
  - ' (This response is sponsored by Free One API. Consider star the project on GitHub:
    https://github.com/RockChinQ/free-one-api )'
  # Enable random ad
  enabled: false
  # Random ad rate
  rate: 0.05
router:
  # Backend listen port
  port: 3000
  # Admin page login password
  token: '12345678'
watchdog:
  heartbeat:
    # Max fail times
    fail_limit: 3
    # Heartbeat check interval (seconds)
    interval: 1800
    # Single channel heartbeat check timeout (seconds)
    timeout: 300
web:
  # Frontend page path
  frontend_path: ./web/dist/
```