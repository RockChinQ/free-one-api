<script setup>
import { ref, reactive, onMounted } from "vue";
import axios from "axios";

const contentContainerWidth = ref("1000px");

function recalcContentContainerWidth() {
    contentContainerWidth.value =
        document.documentElement.clientWidth > 1000 ? "1000px" : "100%";

    console.log(contentContainerWidth.value);
}

const version_str = ref("")

function getVersion(){
    axios.get("/api/info/version").then((res) => {
        version_str.value = res.data.data
    })
}

onMounted(() => {
    recalcContentContainerWidth();
    getVersion()
});

onresize = () => {
    recalcContentContainerWidth();
};

</script>

<template>
<div id="overall_container">
    <div id="content_container" :style="{ width: contentContainerWidth }">
        <div id="content">
            <div id="content_header">
                <h1>free-one-api</h1>
                <span v-if="version_str!=''" id="version_label">{{ version_str }}</span>
            </div>
            <div id="content_body">
                <p>
                    Makes reverse engineering LLM libs a OpenAI format API.
                </p>
                <p>
                    The source code and documents of this project are available on 
                    <a href="https://github.com/RockChinQ/free-one-api">
                        GitHub
                    </a>
                </p>
                <p>
                    Built by <a href="https://rockchin.top">RockChinQ</a>
                </p>
            </div>
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

#content_container {
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
    border-radius: 5px;
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
}

#version_label {
    position: absolute;
    top: 0;
    right: 0;
    margin: 0.6rem;
    font-size: 0.8rem;
    font-weight: bold;
    color: #ffffff;
    background-color: #348de5;
    padding-inline: 0.4rem;
    border-radius: 0.2rem;
    padding-block: 0.1rem;
}
</style>
