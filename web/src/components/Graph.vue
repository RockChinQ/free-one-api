<script setup>

import axios from 'axios';
import { ref } from 'vue';
import * as echarts from 'echarts';

const records = ref([]);

function refreshRecords(){
    // axios
}

var waterfall = echarts.init(document.getElementById('waterfall'));

</script>

<template>
    <div id="overall_container">
        <div id="waterfall"></div>
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
</style>
