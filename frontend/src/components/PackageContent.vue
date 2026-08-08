<script setup lang="ts">
// P9.6: renders a package's archive inventory (P5.8) — file types, sizes,
// and directory structure — or a "not analyzed" state when no inventory
// enrichment exists yet (P5.2's invariant: enrichment outlives the blob, so
// this never needs the archive present).
import { computed, ref, watch } from "vue";
import { useBamClient } from "../composables/useBamClient";
import type { Inventory } from "../generated/types";

const props = defineProps<{ packageId: number | null }>();

const client = useBamClient();
const inventory = ref<Inventory | null>(null);

async function load() {
  if (props.packageId == null) {
    inventory.value = null;
    return;
  }
  const res = await client.getInventory({ package_id: props.packageId });
  inventory.value = res.inventory ?? null;
}

watch(() => props.packageId, load, { immediate: true });

const byKind = computed(() => {
  const kinds = new Map<string, { count: number; size: number }>();
  for (const f of inventory.value?.files ?? []) {
    const k = kinds.get(f.kind) ?? { count: 0, size: 0 };
    k.count += 1;
    k.size += f.size;
    kinds.set(f.kind, k);
  }
  return [...kinds.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([kind, stats]) => ({ kind, ...stats }));
});

const byDir = computed(() => {
  const dirs = new Map<string, number>();
  for (const f of inventory.value?.files ?? []) {
    const slash = f.path.lastIndexOf("/");
    const dir = slash === -1 ? "." : f.path.slice(0, slash);
    dirs.set(dir, (dirs.get(dir) ?? 0) + 1);
  }
  return [...dirs.entries()].sort(([a], [b]) => a.localeCompare(b));
});
</script>

<template>
  <div class="package-content">
    <p v-if="!inventory" data-testid="not-analyzed" class="not-analyzed">Not analyzed</p>
    <template v-else>
      <ul class="by-kind">
        <li v-for="k in byKind" :key="k.kind" :data-testid="`kind-${k.kind}`">
          {{ k.kind }}: {{ k.count }} files, {{ k.size }} bytes
        </li>
      </ul>
      <ul class="by-dir">
        <li v-for="[dir, count] in byDir" :key="dir" :data-testid="`dir-${dir}`">
          {{ dir }}/ ({{ count }})
        </li>
      </ul>
    </template>
  </div>
</template>
