import Mock from 'mockjs'

// 读取./data/channel/list.json挂到/api

import { channel_list } from './data/channel/list.js'
import { test_channel_latency } from './data/channel/test.js'
import { channel_details } from './data/channel/details.js'

Mock.mock("/api/channel/list", "get", channel_list)
// /api/channel/test/<id:int>
Mock.mock(/\/api\/channel\/test\/(\d+)/, "post", test_channel_latency)
Mock.mock(/\/api\/channel\/details\/(\d+)/, "get", channel_details)

import { adapter_list } from './data/adapter/list.js'

Mock.mock("/api/adapter/list", "get", adapter_list)

import { key_list } from './data/key/list.js'

Mock.mock("/api/key/list", "get", key_list)

import { key_raw } from './data/key/raw.js'

Mock.mock(/\/api\/key\/raw\/(\d+)/, "get", key_raw)

import { create_result } from './data/key/create.js'

Mock.mock("/api/key/create", "post", create_result)

Mock.mock("/check_password", "post", {
    "code": 0,
    "message": "ok",
})
