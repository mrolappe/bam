<script setup lang="ts">
import { ref } from "vue";
import QueryInput from "./components/QueryInput.vue";
import PackageList from "./components/PackageList.vue";
import PackageDetail from "./components/PackageDetail.vue";
import type { Predicate } from "./generated/types";

const predicate = ref<Predicate>({ FullText: "" });
const selectedId = ref<number | null>(null);
</script>

<template>
  <QueryInput @predicate="predicate = $event" />
  <div class="browser">
    <PackageList :predicate="predicate" @select="selectedId = $event" />
    <PackageDetail :package-id="selectedId" />
  </div>
</template>
