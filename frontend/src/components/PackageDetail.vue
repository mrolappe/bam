<script setup lang="ts">
// P9.4: detail pane for the currently selected package.
import { ref, watch } from "vue";
import { useBamClient } from "../composables/useBamClient";
import type { Package } from "../generated/types";

const props = defineProps<{ packageId: number | null }>();

const client = useBamClient();
const pkg = ref<Package | null>(null);

async function load() {
  if (props.packageId == null) {
    pkg.value = null;
    return;
  }
  const res = await client.getPackage({ id: props.packageId });
  pkg.value = res.package ?? null;
}

watch(() => props.packageId, load, { immediate: true });
</script>

<template>
  <div v-if="pkg" class="package-detail">
    <h2>{{ pkg.name }}</h2>
    <dl>
      <dt>Path</dt>
      <dd>{{ pkg.dir }}/{{ pkg.file }}</dd>
      <template v-if="pkg.version">
        <dt>Version</dt>
        <dd>{{ pkg.version }}</dd>
      </template>
      <template v-if="pkg.uploaded_on">
        <dt>Uploaded</dt>
        <dd>{{ pkg.uploaded_on }} ({{ pkg.date_precision }})</dd>
      </template>
      <template v-if="pkg.size_bytes != null">
        <dt>Size</dt>
        <dd>{{ pkg.size_bytes }} bytes</dd>
      </template>
      <template v-if="pkg.description">
        <dt>Description</dt>
        <dd>{{ pkg.description }}</dd>
      </template>
    </dl>
  </div>
</template>
