# Config File

The config lives in data/config.yaml (/app/data/config.yaml inside the container) and is generated on first start. Changes take effect after a restart.

```yaml
# Global defaults per adapter (used as fallback for channel Config)
adapters:
  acheong08_ChatGPT:
    # Automatically ignore duplicated responses
    auto_ignore_duplicated: false
    # Reverse proxy address; the public default is under heavy load, build your own
    reverse_proxy: https://chatproxy.rockchin.top/api/
```

  # Defaults for the DS_WebAgent forwarding adapter
  ds-webagent/deepseek-web:
    base_url: http://ds-webagent:9178
    api_token: local-dev-token
    default_model: ds-web-search
    timeout_secs: 900

database:
  # SQLite file holding channels, keys and logs
  path: ./data/free_one_api.db
  type: sqlite

logging:
  debug: false          # set true for debug output

# Random ad: appended to the end of a response with a given probability
random_ad:
  enabled: false
  rate: 0.05
  ad_list:
  - ' (This response is sponsored by Free One API. Consider star the project on GitHub:
    https://github.com/RockChinQ/free-one-api )'

router:
  port: 3000            # listen port (admin page and API share it)
  token: '12345678'     # admin page login password

watchdog:
  heartbeat:
    fail_limit: 3       # consecutive failures before a channel is disabled
    interval: 1800      # heartbeat interval, seconds
    timeout: 300        # per-channel heartbeat timeout, seconds

web:
  frontend_path: ./web/dist/   # built frontend directory


## Section by section

### adapters

Global config per adapter. A channel's own Config wins: if the channel sets a value it is used, otherwise this section, otherwise the adapter's built-in default.

### database

SQLite only. The directory of path must be writable and persisted, otherwise channels and keys disappear on restart.

### router.token

The admin page password. Change it in production, and do not use a weak one — the admin page can read every API key and channel credential.

### watchdog.heartbeat

A background task calls each enabled channel's test() periodically:

- interval — how often it runs;
- timeout — per-run timeout;
- fail_limit — consecutive failures before the channel is disabled (re-enable it by hand afterwards).

> While DS_WebAgent is not logged in to DeepSeek, /health returns ready: false, which counts as a failure — log in first, then re-enable the channel.

### random_ad

Appends one random line from ad_list to the end of a response with probability rate. Set enabled: false to turn it off.

### web.frontend_path

Directory of the built frontend. Manual installs need npm run build in web/; the repository Dockerfile already builds it inside the image.

## Environment variables

Free One API itself has no environment overrides; configuration is read from data/config.yaml only. Environment variables related to DS_WebAgent (such as DSWA_API_TOKEN) belong to that container — see DS_WebAgent.
