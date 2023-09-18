<script setup>

import { ref, reactive } from 'vue';

// axios
import axios from 'axios';
import { onMounted, computed } from 'vue';
import {
    Delete,
    Refresh,
    DocumentAdd,
    Timer
} from '@element-plus/icons-vue'
import { ElNotification } from 'element-plus'

const channelList = ref([]);

function refreshChannelList() {
    axios.get('/api/channel/list')
        .then(res => {
            console.log(res);
            channelList.value = res.data.data;
            ElNotification({
                message: 'Successfully refreshed channel list.',
                type: 'success',
                duration: 1800
            })
        })
        .catch(err => {
            console.log(err);
            ElNotification({
                message: 'Failed to refresh channel list.',
                type: 'error'
            })
        })
}

function recalcChannelContainerWidth() {
    channelContainerWidth.value = document.documentElement.clientWidth > 1000 ? '1000px' : document.documentElement.clientWidth + 'px';

    console.log(channelContainerWidth.value);
}

onMounted(() => {
    refreshChannelList();
    recalcChannelContainerWidth();
    getUsableAdapterList();
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

function deleteChannelConfirmed(channel_id) {
    console.log(channel_id);
    axios.delete('/api/channel/delete/' + channel_id)
        .then(res => {
            console.log(res);
            if (res.data.code == 0) {
                ElNotification({
                    message: 'Successfully deleted channel.',
                    type: 'success'
                })
            } else {
                ElNotification({
                    message: 'Failed: ' + res.data.message,
                    type: 'error'
                })
            }
        })
        .catch(err => {
            console.log(err);
            ElNotification({
                message: 'Failed to delete channel.',
                type: 'error'
            })
        })
        .finally(() => {
            refreshChannelList();
        })
}

function testChannelLatancy(channel_id) {
    console.log(channel_id);

    axios.post('/api/channel/test/' + channel_id)
        .then(res => {
            console.log(res);

            if (res.data.code == 0) {
                
                for (let i = 0; i < channelList.value.length; i++) {
                    if (channelList.value[i].id == channel_id) {
                        channelList.value[i].latency = res.data.data.latency;
                    }
                }
            } else {
                ElNotification({
                    message: 'Failed: ' + res.data.message,
                    type: 'error'
                })
                
                for (let i = 0; i < channelList.value.length; i++) {
                    if (channelList.value[i].id == channel_id) {
                        channelList.value[i].latency = -1;
                    }
                }
            }
        })
        .catch(err => {
            console.log(err);
            ElNotification({
                message: 'Failed to test channel.',
                type: 'error'
            })
            for (let i = 0; i < channelList.value.length; i++) {
                if (channelList.value[i].id == channel_id) {
                    channelList.value[i].latency = -1
                }
            }
        })
}

function testAllChannelLatancy() {
    for (let i = 0; i < channelList.value.length; i++) {
        testChannelLatancy(channelList.value[i].id);
    }
}

function enableChannel(channel_id) {
    axios.post('/api/channel/enable/' + channel_id)
        .then(res => {
            console.log(res);
            if (res.data.code == 0) {
                ElNotification({
                    message: 'Successfully enabled channel.',
                    type: 'success'
                })
                refreshChannelList();
            } else {
                ElNotification({
                    message: 'Failed: ' + res.data.message,
                    type: 'error'
                })
            }
        })
        .catch(err => {
            console.log(err);
            ElNotification({
                message: 'Failed to enable channel.',
                type: 'error'
            })
        })
}

function disableChannel(channel_id) {
    axios.post('/api/channel/disable/' + channel_id)
        .then(res => {
            console.log(res);
            if (res.data.code == 0) {
                ElNotification({
                    message: 'Successfully disabled channel.',
                    type: 'success'
                })
                refreshChannelList();
            } else {
                ElNotification({
                    message: 'Failed: ' + res.data.message,
                    type: 'error'
                })
            }
        })
        .catch(err => {
            console.log(err);
            ElNotification({
                message: 'Failed to disable channel.',
                type: 'error'
            })
        })
}

// channel details/creation dialog
const detailsDialogVisible = ref(false);
const showingChannelIndex = ref(-1); // -1 when creating a new channel
const showingChannelData = reactive({
    "details": {
        "id": "0", // -1 if this is a new channel
        "name": "name_of_this_channel",
        "adapter": {
            "type": "adapter_name", // get from /api/adapter/list
            "config": `{}` // configuration
        },
        "model_mapping": `{
            "reqModelName": "targetModelName"
        }`,
        "enabled": true, // no need for creation
        "latency": 0.13 // no need for creation
    }
});
const usableAdapterList = ref([
    {
        "name": "acheong08/ChatGPT",
        "config_comment": "this is the comment"
    },
    {
        "name": "xtekky/gpt4free",
        "config_comment": "this is the comment"
    },
    {
        "name": "KoushikNavuluri/Claude-API",
        "config_comment": "this is the comment"
    },
    {
        "name": "Soulter/hugging-chat-api",
        "config_comment": "this is the comment"
    },
    {
        "name": "acheong08/EdgeGPT",
        "config_comment": "this is the comment"
    }
]);
const usableAdapterMap = ref({
    "acheong08/ChatGPT": {
        "name": "acheong08/ChatGPT",
        "config_comment": "this is the comment"
    },
})

function getUsableAdapterList() {
    axios.get('/api/adapter/list')
        .then(res => {
            console.log(res);
            usableAdapterList.value = res.data.data;

            for (let i = 0; i < usableAdapterList.value.length; i++) {
                usableAdapterMap.value[usableAdapterList.value[i].name] = usableAdapterList.value[i];
            }
        })
        .catch(err => {
            console.log(err);
            ElNotification({
                message: 'Failed to get usable adapter list.',
                type: 'error'
            })
        })
}

function showDetails(channel_id) {
    for (let i = 0; i < channelList.value.length; i++) {
        if (channelList.value[i].id == channel_id) {
            // get the details of the channel

            axios.get('/api/channel/details/' + channel_id)
                .then(res => {
                    console.log(res);
                    showingChannelData.details = res.data.data;
                    showingChannelIndex.value = i;
                    detailsDialogVisible.value = true;

                    showingChannelData.details.adapter.config = JSON.stringify(showingChannelData.details.adapter.config, null, 4);
                    showingChannelData.details.model_mapping = JSON.stringify(showingChannelData.details.model_mapping, null, 4);
                })
                .catch(err => {
                    console.log(err);
                    ElNotification({
                        message: 'Failed to get channel details.',
                        type: 'error'
                    })
                })

            break;
        }
    }
}

function showCreateChannelDialog() {
    showingChannelIndex.value = -1;
    showingChannelData.details = {
        "id": "0", // -1 if this is a new channel
        "name": "",
        "adapter": {
            "type": "acheong08/ChatGPT", // get from /api/adapter/list
            "config": `{}` // configuration
        },
        "model_mapping": `{}`,
        "enabled": true, // no need for creation
        "latency": 0.13 // no need for creation
    };
    detailsDialogVisible.value = true;
}

function validateChannel() {
    if (showingChannelData.details.model_mapping == "") {
        showingChannelData.details.model_mapping = `{}`
    }

    if (showingChannelData.details.adapter.type == "") {
        ElNotification({
            message: 'Please select an adapter.',
            type: 'error'
        })
        return false;
    }

    if (showingChannelData.details.adapter.config == "") {
        showingChannelData.details.adapter.config = `{}`
    }

    // check if model_mapping and  adapter.config are valid json
    try {
        showingChannelData.details.model_mapping = JSON.parse(showingChannelData.details.model_mapping);
    } catch (e) {
        ElNotification({
            message: 'Model mapping is not a valid JSON.',
            type: 'error'
        })
        return false;
    }

    try {
        showingChannelData.details.adapter.config = JSON.parse(showingChannelData.details.adapter.config);
    } catch (e) {
        ElNotification({
            message: 'Adapter config is not a valid JSON.',
            type: 'error'
        })
        return false;
    }

    if (showingChannelData.details.name == "") {
        ElNotification({
            message: 'Channel name is required.',
            type: 'error'
        })
        return false;
    }

    return true;
}

function applyChannelDetails() {
    // channelList.value[showingChannelIndex.value].name = showingChannelData.details.name;
    // channelList.value[showingChannelIndex.value].adapter = showingChannelData.details.adapter.type;
    // channelList.value[showingChannelIndex.value].model_mapping = showingChannelData.details.model_mapping;
    // channelList.value[showingChannelIndex.value].enabled = showingChannelData.details.enabled;
    // channelList.value[showingChannelIndex.value].latency = showingChannelData.details.latency;
    console.log(showingChannelData);

    if (!validateChannel()) {
        return;
    }

    if (showingChannelIndex.value == -1) { // create new
        axios.post('/api/channel/create', showingChannelData.details)
            .then(res => {
                console.log(res);

                if (res.data.code == 0) {

                    ElNotification({
                        message: 'Successfully created channel.',
                        type: 'success'
                    })
                    detailsDialogVisible.value = false;
                    refreshChannelList();
                } else {
                    ElNotification({
                        message: 'Failed: ' + res.data.message,
                        type: 'error'
                    })
                }
            })
            .catch(err => {
                console.log(err);
                ElNotification({
                    message: 'Failed to create channel.',
                    type: 'error'
                })
            })
            .finally(() => {
                // reset model_mapping and adapter.config to string
                showingChannelData.details.model_mapping = JSON.stringify(showingChannelData.details.model_mapping, null, 4);
                showingChannelData.details.adapter.config = JSON.stringify(showingChannelData.details.adapter.config, null, 4);
            })
    } else {
        axios.put('/api/channel/update/' + showingChannelData.details.id, showingChannelData.details)
            .then(res => {
                console.log(res);
                if (res.data.code == 0) {
                    ElNotification({
                        message: 'Successfully updated channel.',
                        type: 'success'
                    })
                    detailsDialogVisible.value = false;
                    refreshChannelList();
                } else {
                    ElNotification({
                        message: 'Failed: ' + res.data.message,
                        type: 'error'
                    })
                }
            })
            .catch(err => {
                console.log(err);
                ElNotification({
                    message: 'Failed to update channel.',
                    type: 'error'
                })
            })
            .finally(() => {
                // reset model_mapping and adapter.config to string
                showingChannelData.details.model_mapping = JSON.stringify(showingChannelData.details.model_mapping, null, 4);
                showingChannelData.details.adapter.config = JSON.stringify(showingChannelData.details.adapter.config, null, 4);
            })
    }
}

</script>

<template>
    <div id="overall_container">
        <div id="channel_operation_bar" :style="{ width: channelContainerWidth }">
            <el-button type="success" :icon="DocumentAdd" @click="showCreateChannelDialog">Add</el-button>
            <el-button type="success" :icon="Refresh" @click="refreshChannelList">Refresh</el-button>
            <el-button type="success" :icon="Timer" @click="testAllChannelLatancy">Test All</el-button>
        </div>
        <div id="channel_list_container" :style="{ width: channelContainerWidth }">
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
                    <text class="channel_adapter_box" :style="{ 'border-left-color': adapter_color[channel.adapter] }">{{
                        channel.adapter }}</text>
                </text>
                <text class="channel_latency">
                    <el-button class="channel_latency_box" @click="testChannelLatancy(channel.id)"
                        :type="channel.latency > 0 ? 'success' : 'default'" plain>{{ channel.latency > 0 ? channel.latency +
                            's' :
                            'N/A' }}</el-button>
                </text>
                <span class="op_container">
                    <!-- <el-button class="op_test" plain>Test</el-button> -->
                    <el-button class="op_switch" :type="!channel.enabled ? 'success' : 'danger'"
                        @click="channel.enabled ? disableChannel(channel.id) : enableChannel(channel.id)">{{ channel.enabled
                            ?
                            'Disable' : 'Enable' }}</el-button>
                    <el-button class="op_edit" @click="showDetails(channel.id)">{{ 'Edit' }}</el-button>
                    <el-popconfirm title="Are you sure to delete this channel?"
                        @confirm="deleteChannelConfirmed(channel.id)">
                        <template #reference>
                            <el-button class="op_delete" :icon="Delete" />
                        </template>
                    </el-popconfirm>
                </span>
            </div>
        </div>

        <!-- channel details/new dialog -->
        <el-dialog v-model="detailsDialogVisible"
            :title="showingChannelIndex >= 0 ? 'Channel #' + channelList[showingChannelIndex].id : 'New Channel'">
            <el-form :model="showingChannelData.details">
                <el-form-item label="Name">
                    <el-input v-model="showingChannelData.details.name" />
                </el-form-item>
                <el-form-item label="Adapter">
                    <el-select v-model="showingChannelData.details.adapter.type">
                        <el-option v-for="adapter in usableAdapterList" :key="adapter.name" :label="adapter.name"
                            :value="adapter.name" />
                    </el-select>
                </el-form-item>

                <!--Adapter configuration-->
                <el-form-item label="Config">
                    <el-input v-model="showingChannelData.details.adapter.config" rows="8" type="textarea" />
                    <el-popover placement="bottom" :title="showingChannelData.details.adapter.type" :width="700" trigger="click">
                        <template #reference>
                            <el-button class="m-2">Config Comment</el-button>
                        </template>
                        <pre>{{ usableAdapterMap[showingChannelData.details.adapter.type].config_comment }}</pre>
                    </el-popover>
                </el-form-item>
                <!-- config comment, not editable -->

                <el-form-item label="Model Mapping">
                    <el-input v-model="showingChannelData.details.model_mapping" rows="5" type="textarea" />
                </el-form-item>
            </el-form>

            <template #footer>
                <span class="dialog-footer">
                    <el-button @click="detailsDialogVisible = false">Cancel</el-button>
                    <el-button type="primary" @click="applyChannelDetails">Confirm</el-button>
                </span>
            </template>
        </el-dialog>
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
    flex-direction: column;
}

#channel_operation_bar {
    position: relative;
    display: flex;
    margin-top: 0.4rem;
    align-items: center;
    justify-content: flex-start;
    width: 100%;
}

#channel_list_container {
    position: relative;
    top: 0;
    left: 0;
    margin-top: 0.6rem;
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
    font-size: 0.9rem;
    width: 20%;
    /* font-weight: bold; */
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

.channel_latency_box {
    position: relative;
    /* border: 2px solid rgb(87, 218, 87); */
    font-size: 1rem;
    width: 90%;
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
