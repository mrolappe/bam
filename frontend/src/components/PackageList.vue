<script setup lang="ts">
// P9.4: a virtualized package list. Renders only the rows within
// `viewportHeight` of the current scroll offset (plus a small overscan), so
// mounting a search result with tens of thousands of rows still only ever
// creates a bounded number of `<li>`s.
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useBamClient } from "../composables/useBamClient";
import type { Package, Predicate } from "../generated/types";

const props = withDefaults(
  defineProps<{
    predicate?: Predicate;
    itemHeight?: number;
    viewportHeight?: number;
  }>(),
  {
    predicate: () => ({ FullText: "" }),
    itemHeight: 32,
    viewportHeight: 480,
  },
);

const emit = defineEmits<{
  select: [id: number];
}>();

const client = useBamClient();
const packages = ref<Package[]>([]);
const marked = reactive(new Set<number>());
const scrollTop = ref(0);

async function load() {
  const res = await client.searchPackages({ predicate: props.predicate });
  packages.value = res.packages;
}

onMounted(load);
watch(() => props.predicate, load);

// ponytail: viewportHeight is a fixed prop rather than a ResizeObserver on
// the scroll container — simplest thing that lets both jsdom tests and a
// real browser agree on how many rows are "visible"; swap in an observer if
// the list ever needs to fill an unknown container height.
const overscan = 4;
const visibleRowCount = computed(
  () => Math.ceil(props.viewportHeight / props.itemHeight) + overscan * 2,
);
const startIndex = computed(() =>
  Math.max(0, Math.floor(scrollTop.value / props.itemHeight) - overscan),
);
const endIndex = computed(() =>
  Math.min(packages.value.length, startIndex.value + visibleRowCount.value),
);
const visiblePackages = computed(() => packages.value.slice(startIndex.value, endIndex.value));
const topPad = computed(() => startIndex.value * props.itemHeight);
const bottomPad = computed(() => (packages.value.length - endIndex.value) * props.itemHeight);

function onScroll(e: Event) {
  scrollTop.value = (e.target as HTMLElement).scrollTop;
}

async function toggleMark(id: number) {
  const isMarked = await client.toggle(id);
  if (isMarked) marked.add(id);
  else marked.delete(id);
}
</script>

<template>
  <div class="package-list" :style="{ height: `${viewportHeight}px`, overflowY: 'auto' }" @scroll="onScroll">
    <div :style="{ height: `${topPad}px` }" />
    <ul>
      <li
        v-for="pkg in visiblePackages"
        :key="pkg.id"
        :style="{ height: `${itemHeight}px` }"
        @click="emit('select', pkg.id)"
      >
        <button
          type="button"
          :data-testid="`mark-${pkg.id}`"
          @click.stop="toggleMark(pkg.id)"
        >{{ marked.has(pkg.id) ? "✓" : "○" }}</button>
        {{ pkg.name }}
      </li>
    </ul>
    <div :style="{ height: `${bottomPad}px` }" />
  </div>
</template>
