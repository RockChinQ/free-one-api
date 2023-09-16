function test_channel_latency() {
    return {
        "code": 0,
        "message": "ok",
        "data": {
            "latency": parseInt(Math.random()*100)/100
        }
    }
}

export {
    test_channel_latency
}