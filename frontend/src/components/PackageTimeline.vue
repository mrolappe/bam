<script setup lang="ts">
// P9.5: upload counts per year for the active query, `exact` and `week`
// (P1.2) precision kept as separate stacked segments so a `week`-precision
// point is never drawn as though its date were certain.
import { computed, onMounted, ref, watch } from "vue";
import { useBamClient } from "../composables/useBamClient";
import type { Package, Predicate } from "../generated/types";

const props = withDefaults(defineProps<{ predicate?: Predicate }>(), {
  predicate: () => ({ FullText: "" }),
});

const client = useBamClient();
const packages = ref<Package[]>([]);

async function load() {
  const res = await client.searchPackages({ predicate: props.predicate });
  packages.value = res.packages;
}

onMounted(load);
watch(() => props.predicate, load);

const buckets = computed(() => {
  const byYear = new Map<string, { exact: number; week: number }>();
  for (const pkg of packages.value) {
    if (!pkg.uploaded_on) continue;
    const year = pkg.uploaded_on.slice(0, 4);
    const bucket = byYear.get(year) ?? { exact: 0, week: 0 };
    if (pkg.date_precision === "week") bucket.week += 1;
    else bucket.exact += 1;
    byYear.set(year, bucket);
  }
  return [...byYear.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([year, counts]) => ({ year, ...counts, total: counts.exact + counts.week }));
});
</script>

<template>
  <div class="package-timeline">
    <div v-for="b in buckets" :key="b.year" :data-testid="`year-${b.year}`" class="year-bucket">
      <span class="year-label">{{ b.year }}</span>
      <span class="count-label">{{ b.total }}</span>
      <div
        v-if="b.exact > 0"
        :data-testid="`bar-exact-${b.year}`"
        class="bar precision-exact"
        :style="{ width: `${b.exact * 8}px` }"
      />
      <div
        v-if="b.week > 0"
        :data-testid="`bar-week-${b.year}`"
        class="bar precision-week"
        :style="{ width: `${b.week * 8}px` }"
      />
    </div>
  </div>
</template>
