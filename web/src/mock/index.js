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
