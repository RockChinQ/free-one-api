<script setup>
import { ref, reactive, onMounted } from "vue";
import axios from "axios";

const contentContainerWidth = ref("1000px");

function recalcContentContainerWidth() {
    contentContainerWidth.value =
        document.documentElement.clientWidth > 1000 ? "1000px" : "100%";

    console.log(contentContainerWidth.value);
}

onMounted(() => {
    recalcContentContainerWidth();
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
</style>
