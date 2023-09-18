function channel_details() {
    return {
        code: 0,
        message: "ok",
        data: {
            id: 1,
            name: "revChatGPT",
            adapter: {
                type: "acheong08/ChatGPT",
                config: {
                    paid: true,
                    access_token: "revchatgpt_access_token",
                },
            },
            model_mapping: {},
            enabled: true,
            latency: 0.6,
        },
    };
}

export { channel_details };
