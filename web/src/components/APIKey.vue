<script setup>
import { ref, reactive, onMounted } from "vue";
import axios from "axios";

import { ElNotification, ElMessageBox } from "element-plus";

import { Delete, Refresh, DocumentAdd, Timer } from "@element-plus/icons-vue";

import { copyText } from "vue3-clipboard";

const keyContainerWidth = ref("1000px");

const keyList = ref([]);

function refreshKeyList() {
    axios
        .get("/api/key/list")
        .then((res) => {
            console.log(res.data);
            keyList.value = res.data.data;
            // 把每个key的created_at转换为本地时间
            keyList.value.forEach((key) => {
                key.created_at = new Date(key.created_at).toLocaleString();
            });

            ElNotification({
                message: "Successfully refreshed key list.",
                type: "success",
                duration: 1000,
            });
        })
        .catch((err) => {
            console.log(err);
            ElNotification({
                message: "Failed to refresh key list.",
                type: "error",
            });
        });
}

function recalcKeyContainerWidth() {
    keyContainerWidth.value =
        document.documentElement.clientWidth > 1000 ? "1000px" : "100%";

    console.log(keyContainerWidth.value);
}

onMounted(() => {
    refreshKeyList();
    recalcKeyContainerWidth();
});

onresize = () => {
    recalcKeyContainerWidth();
};

function copyKey(key_id) {
    // copy key to paste board
    console.log(key_id);

    axios
        .get("/api/key/raw/" + key_id)
        .then((res) => {
            console.log(res.data);

            if (res.data.code !== 0) {
                ElNotification({
                    message: "Failed: "+res.data.message,
                    type: "error",
                });
                return;
            } else {
                ElMessageBox.alert(res.data.data.key, 'API Key', {
                    confirmButtonText: 'OK',
                })
            }
        })
        .catch((err) => {
            console.log(err);
            ElNotification({
                message: "Failed to copy key to paste board.",
                type: "error",
            });
        });
}

function createKey(){
    ElMessageBox.prompt(
        "Please enter the name of your key:",
        "Add Key",
        {
            confirmButtonText: "OK",
            cancelButtonText: "Cancel",
            // A-Z a-z 0-9 - _ . @
            inputPattern: /^[A-Za-z0-9\-_\.@]+$/,
            inputErrorMessage: "Invalid key name.(A-Z a-z 0-9 - _ . @ only)",
        }
    ).then(
        ({value}) => {
            let key_name = value;
            console.log(key_name);
            axios.post("/api/key/create", {
                name: key_name,
            }).then(
                (res) => {
                    console.log(res.data);
                    if (res.data.code !== 0) {
                        ElNotification({
                            message: "Failed: "+res.data.message,
                            type: "error",
                        });
                        return;
                    } else {
                        refreshKeyList();

                        // view this key
                        ElMessageBox.alert(res.data.data.raw, 'API Key', {
                            confirmButtonText: 'OK',
                        })
                    }
                }
            ).catch(
                (err) => {
                    console.log(err);
                    ElNotification({
                        message: "Failed to create key.",
                        type: "error",
                    });
                }
            )
        }
    ).catch(
        () => {}
    )
}

function deleteKeyConfirmed(key_id){
    axios.delete("/api/key/revoke/"+key_id).then(
        (res) => {
            console.log(res.data);
            if (res.data.code !== 0) {
                ElNotification({
                    message: "Failed: "+res.data.message,
                    type: "error",
                });
                return;
            } else {
                refreshKeyList();
                ElNotification({
                    message: "Successfully deleted key.",
                    type: "success",
                });
            }
        }
    ).catch(
        (err) => {
            console.log(err);
            ElNotification({
                message: "Failed to delete key.",
                type: "error",
            });
        }
    )
}
</script>

<template>
    <div id="overall_container">
        <div id="key_operation_bar" :style="{ width: keyContainerWidth}">
            <el-button type="success" :icon="DocumentAdd" @click="createKey">Add</el-button>
            <el-button type="success" :icon="Refresh" @click="refreshKeyList">Refresh</el-button>
        </div>
        <div id="key_list_container" :style="{ width: keyContainerWidth }">
            <div class="key">
                <text class="key_id key_title_text">ID</text>
                <text class="key_name key_title_text">Name</text>
                <text class="key_brief key_title_text">Brief</text>
                <text class="key_created_at key_title_text">Create Time</text>
                <span class="op_container key_title_text">
                    Operation
                </span>
            </div>
            <div class="key" v-for="key in keyList" :key="key.id">
                <text class="key_id">{{ key.id }}</text>
                <text class="key_name">{{ key.name }}</text>
                <text class="key_brief">{{ key.brief }}</text>
                <text class="key_created_at">{{ key.created_at }}</text>
                <span class="op_container">
                    <el-button type="primary" plain @click="copyKey(key.id)">View Key</el-button>
                    <el-popconfirm title="Are you sure to delete this key?"
                        @confirm="deleteKeyConfirmed(key.id)">
                        <template #reference>
                            <el-button class="op_delete" type="danger" :icon="Delete" />
                        </template>
                    </el-popconfirm>
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
    flex-direction: column;
}

#key_operation_bar {
    position: relative;
    display: flex;
    margin-top: 0.4rem;
    align-items: center;
    justify-content: flex-start;
    width: 100%;
}

#key_list_container {
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

.key {
    position: relative;
    margin-block: 0.2rem;
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
    width: 100%;
    border-radius: 0.2rem;
    display: flex;
    padding-block: 0.5rem;
}

.key_id {
    margin-left: 1rem;
    font-size: 1.1rem;
    font-weight: bold;
    width: 3%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
}

.key_name {
    margin-left: 1rem;
    font-size: 0.9rem;
    width: 20%;
    /* font-weight: bold; */
    display: flex;
    align-items: center;
    justify-content: flex-start;
}

.key_brief {
    margin-left: 1rem;
    font-size: 0.9rem;
    width: 32%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
}

.key_created_at {
    margin-left: 1rem;
    font-size: 0.9rem;
    width: 23%;
    display: flex;
    font-weight: bold;
    align-items: center;
    justify-content: flex-start;
}

.op_container {
    margin-left: 1rem;
    font-size: 0.9rem;
    width: 20%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
}

.key_title_text {
    font-size: 0.8rem;
    font-weight: bold;
    text-align: center;
}
</style>
