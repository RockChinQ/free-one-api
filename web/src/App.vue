<script setup>
import Home from './components/Home.vue';
import Channel from './components/Channel.vue';

import { ref } from 'vue';

const currentTab = ref('home');

</script>

<template>
    <div id="topbar_container" class="flex_container">
        <div id="logo_container" class="flex_container">
            <a class="no_url_style flex_container" href="/">
                <img id="logo" src="./assets/logo.png" alt="logo">
                <text id="project_name">Free One API</text></a>
        </div>
        <div class="tab_btn flex_container" @click="currentTab = 'home'">Home</div>
        <div class="tab_btn flex_container" @click="currentTab = 'channel'">Channels</div>
    </div>

    <div id="content">
        <Home v-if="currentTab === 'home'"></Home>
        <Channel v-if="currentTab === 'channel'"></Channel>
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
