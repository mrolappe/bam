<script setup lang="ts">
// P9.4: bam-dsl query input with P3.5's inline error display — an invalid
// edit shows a message with the offending byte span highlighted and leaves
// the last valid predicate in place, rather than clearing results.
import { computed, ref } from "vue";
import { useBamClient } from "../composables/useBamClient";
import { BamApiError } from "../transport/BamClient";
import type { Predicate } from "../generated/types";

const emit = defineEmits<{
  predicate: [predicate: Predicate];
}>();

const client = useBamClient();
const text = ref("");
const error = ref<BamApiError | null>(null);
let debounceHandle: ReturnType<typeof setTimeout> | undefined;

// Matches P3.5's TUI debounce window (bam-tui/src/app.rs's `DEBOUNCE`), so
// rapid keystrokes coalesce into one query on both frontends.
const DEBOUNCE_MS = 150;

async function commit(src: string) {
  try {
    const res = await client.parseQuery({ src });
    error.value = null;
    emit("predicate", res.predicate);
  } catch (e) {
    error.value = e instanceof BamApiError ? e : new BamApiError(String(e));
  }
}

function onInput(e: Event) {
  text.value = (e.target as HTMLInputElement).value;
  clearTimeout(debounceHandle);
  debounceHandle = setTimeout(() => commit(text.value), DEBOUNCE_MS);
}

const errorSpanParts = computed(() => {
  const span = error.value?.span;
  if (!span) return null;
  const [start, end] = span;
  return {
    before: text.value.slice(0, start),
    span: text.value.slice(start, end),
    after: text.value.slice(end),
  };
});
</script>

<template>
  <div class="query-input">
    <input type="text" :value="text" @input="onInput" placeholder="search…" />
    <p v-if="error" data-testid="query-error">
      <template v-if="errorSpanParts">
        {{ errorSpanParts.before }}<mark data-testid="error-span">{{ errorSpanParts.span }}</mark>{{ errorSpanParts.after }}
      </template>
      {{ error.message }}
    </p>
  </div>
</template>
