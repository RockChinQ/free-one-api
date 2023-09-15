import Mock from 'mockjs'

// 读取./data/channel/list.json挂到/api

import { channel_list } from './data/channel/list.js'

Mock.mock("/api/channel/list", "get", channel_list)