import './mock'

import { createApp } from 'vue'
import App from './App.vue'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'

import VueClipboard from 'vue3-clipboard'

const app = createApp(App)
app.use(ElementPlus)
app.mount('#app')

app.use(VueClipboard)
