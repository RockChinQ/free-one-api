const channel_list = {
    "code": 0,
    "message": "ok",
    "data": [
        {
            "id": 1,
            "name": "revChatGPT",
            "adapter": "acheong08/ChatGPT",
            "enabled": true,
            "latency": 0.6
        },
        {
            "id": 2,
            "name": "revChatGPT",
            "adapter": "acheong08/ChatGPT",
            "enabled": false,
            "latency": -1
        },
        {
            "id": 3,
            "name": "gpt4free",
            "adapter": "xtekky/gpt4free",
            "enabled": true,
            "latency": 0.8
        },
        {
            "id": 4,
            "name": "claude",
            "adapter": "KoushikNavuluri/Claude-API",
            "enabled": false,
            "latency": 1.5
        },
        {
            "id": 5,
            "name": "newbing",
            "adapter": "acheong08/EdgeGPT",
            "enabled": true,
            "latency": 1.5
        },
        {
            "id": 6,
            "name": "hugchat",
            "adapter": "Soulter/hugging-chat-api",
            "enabled": true,
            "latency": 1.5
        }
    ]
}

export {
    channel_list
}