# Configurations

Configuration file is saved at `data/config.yaml`

```yaml
database:
  # SQLite DB file path
  path: ./data/free_one_api.db
  type: sqlite
logging:
  debug: false  # Enable debug log
misc:
  # Reverse proxy address for acheong08/ChatGPT adapter.
  # Default public reverse proxy may be unstable, it is recommended to build your own:
  # https://github.com/acheong08/ChatGPT-Proxy-V4
  chatgpt_api_base: https://chatproxy.rockchin.top/api/
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