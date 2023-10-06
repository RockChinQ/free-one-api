<script setup>
import {ref, onMounted} from "vue";

import axios from "axios";
import { ElNotification, ElMessageBox } from "element-plus";

const logs = ref([]);

const page = ref(0);

const page_count = ref(0);

const keyContainerWidth = ref("1000px");

function refreshLogs(){
    axios.get("/log/list", {
        params: {
            capacity: 20,
            page: page.value
        }
    }).then((response) => {
        console.log(response.data);

        if (response.data.code === 0){
            page_count.value = response.data.data.page_count;
            logs.value = response.data.data.logs;
        }else{
            ElNotification({
                message: "Failed: "+res.data.message,
                type: "error",
            });
        }
    }).catch(
    (err) => {
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
        <el-table id="log_table" :data="logs" stripe >
            <el-table-column prop="id" label="ID" width="80" />
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
