<script setup>
import {ref, onMounted} from "vue";

import axios from "axios";
import { ElNotification, ElMessageBox } from "element-plus";

const logs = ref([]);

const page = ref(0);

const page_count = ref(0);

const keyContainerWidth = ref("1000px");

const loading = ref(false);

const deletingCurrent = ref(false);
const deletingPrevious = ref(false);

function refreshLogs(){
    loading.value = true;
    axios.get("/api/log/list", {
        params: {
            capacity: 10,
            page: page.value
        }
    }).then((response) => {
        console.log(response.data);
        loading.value = false;
        if (response.data.code === 0){
            page_count.value = response.data.data.page_count;

            var logs_raw = response.data.data.logs;

            // 把所有timestamp转换成可读的格式
            for (let i = 0; i < logs_raw.length; i++){
                logs_raw[i].time = new Date(logs_raw[i].timestamp * 1000).toLocaleString();
            }
            logs.value = logs_raw;
        }else{
            ElNotification({
                message: "Failed: "+res.data.message,
                type: "error",
            });
        }
    }).catch(
    (err) => {
        loading.value = false;
        console.log(err);
        ElNotification({
            message: "Failed to fetch logs.",
            type: "error",
        });
    })
}
function recalcKeyContainerWidth() {
    keyContainerWidth.value =
        document.documentElement.clientWidth > 1000 ? "1000px" : "100%";

    console.log(keyContainerWidth.value);
}

function changePage(p){
    page.value = p;
    refreshLogs();
}

function deleteRange(start, end){
    axios.delete("/api/log/delete", {
        params: {
            start: start,
            end: end
        }
    }).then((response) => {
        console.log(response.data);
        if (response.data.code === 0){
            ElNotification({
                message: "Success",
                type: "success",
            });
            refreshLogs();
        }else{
            ElNotification({
                message: "Failed: "+res.data.message,
                type: "error",
            });
        }
        deletingCurrent.value = false;
        deletingPrevious.value = false;
    }).catch(
    (err) => {
        console.log(err);
        ElNotification({
            message: "Failed to delete logs.",
            type: "error",
        });
        deletingCurrent.value = false;
        deletingPrevious.value = false;
    })
}

function deleteCurrentPage(){
    deletingCurrent.value = true;

    var start = logs.value[0].id;
    var end = logs.value[logs.value.length - 1].id;

    deleteRange(end, start);
}

function deletePreviousPages(){
    deletingPrevious.value = true;

    var end = logs.value[logs.value.length - 1].id;

    deleteRange(0, end-1);
}

onMounted(() => {
    refreshLogs();
})

onresize = () => {
    recalcKeyContainerWidth();
};

</script>

<template>
<div id="overall_container">
    <div id="table_container" background :style="{ width: keyContainerWidth}">
        <el-pagination id="pages" layout="prev, pager, next" :page-count="page_count" @current-change="changePage" />
        <span id="operation_bar">
            <el-button v-loading="loading" element-loading-svg-view-box="-25, -25, 100, 100" type="success" @click="refreshLogs()">Refresh</el-button>

            <el-popconfirm title="Are you sure to delete this page of logs?"
                @confirm="deleteCurrentPage()">
                <template #reference>
                    <el-button v-loading="deletingCurrent" element-loading-svg-view-box="-25, -25, 100, 100" type="danger">Delete Current Page</el-button>
                </template>
            </el-popconfirm>
            
            <el-popconfirm title="Are you sure to delete previous pages of logs?"
                @confirm="deletePreviousPages()">
                <template #reference>
                    <el-button v-loading="deletingPrevious" element-loading-svg-view-box="-25, -25, 100, 100" type="danger">Delete Previous Pages</el-button>
                </template>
            </el-popconfirm>
        </span>
        <el-table id="log_table" :data="logs" stripe >
            <el-table-column prop="id" label="ID" width="80" />
            <el-table-column prop="time" label="Time" width="180" />
            <el-table-column prop="content" label="Content" />
        </el-table>
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

#table_container {
    position: relative;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    justify-content: center;
    align-items: center;
    flex-direction: column;
    background-color: white;
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
    border-radius: 0.4rem;
}

#operation_bar{
    position: relative;
    top: 0;
    left: 0;
    margin-left: 10px;
    margin-top: 0.6rem;
    width: 100%;
    height: 100%;
    display: flex;
}

#pages{
    position: relative;
    top: 0;
    left: 0;
    margin-top: 0.6rem;
    width: 100%;
    height: 100%;
    display: flex;
}

#log_table {
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

</style>
