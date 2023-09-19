// import './mock'

import { createApp } from 'vue'
import App from './App.vue'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'

import VueClipboard from 'vue3-clipboard'

import axios from 'axios'
import { getPassword } from './common/account.js'
import md5 from 'js-md5'

axios.interceptors.request.use(function (config) {
    config.headers['Authorization'] = "Bearer " + md5(getPassword());
    console.log(config);
    return config;
})

const app = createApp(App)
app.use(ElementPlus)
app.mount('#app')

app.use(VueClipboard)
