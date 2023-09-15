<script setup>

import { ref } from 'vue';

// axios
import axios from 'axios';
import { onMounted, computed } from 'vue';
import {
  Delete
} from '@element-plus/icons-vue'

const channelList = ref([]);

function refreshChannelList() {
    axios.get('/api/channel/list')
    .then(res => {
        console.log(res);
        channelList.value = res.data.data;
    })
    .catch(err => {
        console.log(err);
    })
}

function recalcChannelContainerWidth() {
    channelContainerWidth.value = document.documentElement.clientWidth > 1000 ? '1000px' : document.documentElement.clientWidth + 'px';

    console.log(channelContainerWidth.value);
}

onMounted(() => {
    refreshChannelList();
    recalcChannelContainerWidth();
});

const channelContainerWidth = ref('1000px');

onresize = () => {
    recalcChannelContainerWidth();
}

const adapter_color = {
    "acheong08/ChatGPT": "#00BB00",
    "xtekky/gpt4free": "#CC33FF",
    "acheong08/EdgeGPT": "#0388FF",
    "Soulter/hugging-chat-api": "#FFBB03",
    "KoushikNavuluri/Claude-API": "#AAAAAA",
}

</script>

<template>

<div id="overall_container">
    <div id="channel_list_container" :style="{width: channelContainerWidth}">
        <div class="chan">
            <text class="channel_id chan_title_text">ID</text>
            <text class="channel_name chan_title_text">Channel Name</text>
            <text class="channel_adapter chan_title_text">Adapter</text>
            <text class="channel_latency chan_title_text">Latency</text>
            <span class="op_container chan_title_text">
                Operation
            </span>
        </div>
        <div class="chan" v-for="channel in channelList">
            <text class="channel_id">{{ channel.id }}</text>
            <text class="channel_name">{{ channel.name }}</text>
            <text class="channel_adapter">
                <text class="channel_adapter_box" :style="{'border-left-color': adapter_color[channel.adapter]}">{{ channel.adapter }}</text>
            </text>
            <text class="channel_latency">{{ channel.latency>0? channel.latency + 's' : 'N/A' }}</text>
            <span class="op_container">
                <!-- <el-button class="op_test" plain>Test</el-button> -->
                <el-button class="op_switch" :type="!channel.enabled? 'success' : 'danger'">{{ channel.enabled? 'Disable' : 'Enable' }}</el-button>
                <el-button class="op_edit">{{ 'Edit' }}</el-button>
                <el-button class="op_delete" :icon="Delete" />
            </span>
        </div>
    </div>
</div>

</template>

<style scoped>

#overall_container {
    position: relative;
    width: 100%;
    height: 100%;
    /* box-shadow: 0 0 10px rgba(0, 0, 0, 0.4); */
    box-sizing: border-box;
    display: flex;
    justify-content: center;
    align-items: center;
}

#channel_list_container {
    position: relative;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    display: flex;
    justify-content: center;
    align-items: center;
    flex-direction: column;
}

.chan {
    position: relative;
    margin-block: 0.2rem;
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
    width: 100%;
    border-radius: 0.2rem;
    display: flex;
    padding-block: 0.5rem;
}

.text_center {
    text-align: center;
}

.channel_id {
    margin-left: 1rem;
    font-size: 1.1rem;
    font-weight: bold;
    width: 3%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
}

.channel_name {
    margin-left: 1rem;
    font-size: 1.1rem;
    width: 20%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
}

.channel_adapter {
    margin-left: 1rem;
    font-size: 0.9rem;
    width: 32%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
}

.channel_adapter_box {
    position: relative;
    /* border: 2px solid rgb(87, 218, 87); */
    border-left-width: 8px;
    border-left-style: solid;
    padding: 0.2rem;
    font-size: 1rem;
}

.channel_latency {
    width: 9%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
}

.op_container {
    position: relative;
    top: 0;
    display: flex;
    align-items: center;
    justify-content: space-evenly;
    /* border: 2px solid; */
    width: 30%;
}

.chan_title_text {
    font-size: 0.8rem;
    font-weight: bold;
    text-align: center;
}

</style>
