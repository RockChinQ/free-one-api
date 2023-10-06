<script setup>
import Home from './components/Home.vue';
import Channel from './components/Channel.vue';
import APIKey from './components/APIKey.vue';
import Log from './components/Log.vue';
import { setPassword, getPassword, clearPassword, checkPassword } from './common/account';

import { ElMessageBox, ElMessage } from 'element-plus';
import { ref } from 'vue';

const currentTab = ref('home');

function showLoginDialog() {
    ElMessageBox.prompt('Please enter your token:', 'Enter token', {
    confirmButtonText: 'OK',
    inputErrorMessage: 'Invalid Format',
    inputType: 'password',
    inputPattern: /\S+/,
  })
    .then(({ value }) => {
        // login
        checkPassword(value)
    })
    .catch(() => {
      ElMessage({
        type: 'info',
        message: 'Input canceled',
      })
    })
}

function switchTab(target){
    if (getPassword() == ""){
        showLoginDialog()
        return
    }

    currentTab.value = target
}

function logout(){
    clearPassword()
    currentTab.value = 'home'
}

</script>

<template>
    <div id="topbar_container">
        <div id="logo_container" class="flex_container">
            <a class="no_url_style flex_container" href="/">
                <img id="logo" src="./assets/logo.png" alt="logo">
                <text id="project_name">Free One API</text></a>
        </div>
        <div class="tab_btn flex_container" @click="switchTab('home')">Home</div>
        <div class="tab_btn flex_container" @click="switchTab('channel')">Channels</div>
        <div class="tab_btn flex_container" @click="switchTab('apikey')">API Keys</div>
        <div class="tab_btn flex_container" @click="switchTab('logs')">Logs</div>
        <div id="login_info">
            <el-button :type="getPassword()==''?'success':'danger'"
            @click="getPassword()==''?showLoginDialog():logout()"
            >
                {{ getPassword()==''?'Login':'Logout' }}
            </el-button>
        </div>
    </div>

    <div id="content">
        <Home v-if="currentTab === 'home'"></Home>
        <Channel v-if="currentTab === 'channel'"></Channel>
        <APIKey v-if="currentTab === 'apikey'"></APIKey>
        <Log v-if="currentTab === 'logs'"></Log>
    </div>
</template>

<style scoped>
#topbar_container {
    position: absolute;
    top: 0.4rem;
    left: 0.4rem;
    width: calc(100% - 0.8rem);
    height: 3.2rem;
    background-color: #fff;
    z-index: 1000;
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.4);
    box-sizing: border-box;
    user-select: none;
    border-radius: 0.5rem;
    display: flex;
    align-items: center;
    justify-content: flex-start;
}

#logo_container {
    position: relative;
    top: 0;
    left: 0;
    height: 100%;
    padding-inline: 0.5rem;
    cursor: pointer;
}

#logo_container:hover {
    background-color: #eee;
}

#logo_container:active {
    background-color: #ddd;
}

#logo {
    width: 2rem;
    height: 2rem;
}

#project_name {
    font-size: 1.1rem;
    font-weight: bold;
    color: #333;
    margin-left: 0.5rem;
}

.tab_btn {
    position: relative;
    margin-inline: 0.3rem;
    padding-inline: 0.5rem;
    font-size: 1.1rem;
    font-weight: bold;
    cursor: pointer;
    height: 100%;
    top: 0;
}

.tab_btn:hover {
    background-color: #eee;
}

.tab_btn:active {
    background-color: #ddd;
}

.no_url_style {
    text-decoration: none;
    color: inherit;
}

.flex_container {
    display: flex;
    align-items: center;
    justify-content: center;
}

#content {
    position: absolute;
    top: 4.2rem;
    left: 0.4rem;
    width: calc(100% - 0.8rem);
}
</style>
