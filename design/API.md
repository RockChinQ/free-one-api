# API Paths

- /api
    - /channel
        - GET /list
        - POST /create
        - DEL /delete/<id:int>
        - GET /details/<id:int>
        - PUT /update/<id:int>
        - POST /enable/<id:int>
        - POST /disable/<id:int>
        - POST /test/<id:int>
    - /adapter
        - GET /list
        - GET /details/<name:string>
    - /key
        - GET /list
        - GET /raw/<id:int>
        - POST /create
        - DEL /revoke/<id:int>
    - /log
        - GET /list
            - page:int capacity:int
    - /info
        - GET /version
    - /statistic
        - /channel
            - GET /records

## Entities

### Channel Detail Data

```JSON
{
    "id": "0", // -1 if this is a new channel
    "name": "name_of_this_channel",
    "adapter": {
        "type": "adapter_name", // get from /api/adapter/list
        "config": {} // configuration
    },
    "model_mapping": {
        "reqModelName": "targetModelName"
    },
    "enabled": true, // no need for creation
    "latency": 0.13 // no need for creation
}
```