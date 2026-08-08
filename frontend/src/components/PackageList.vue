<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useBamClient } from "../composables/useBamClient";
import type { Package } from "../generated/types";

const client = useBamClient();
const packages = ref<Package[]>([]);

onMounted(async () => {
  const res = await client.searchPackages({ predicate: { FullText: "" } });
  packages.value = res.packages;
});
</script>

<template>
  <ul>
    <li v-for="pkg in packages" :key="pkg.id">{{ pkg.name }}</li>
  </ul>
</template>
