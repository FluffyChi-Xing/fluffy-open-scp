<script setup lang="ts">
import { computed } from "vue";
const props = defineProps<{ value: unknown; query: string }>();
const segments = computed(() => {
  const text =
    typeof props.value === "string"
      ? props.value
      : (JSON.stringify(props.value) ?? "");
  const q = props.query.trim().toLowerCase();
  if (!q) return [{ text, match: false }];
  const result: { text: string; match: boolean }[] = [];
  let offset = 0;
  while (offset < text.length) {
    const index = text.toLowerCase().indexOf(q, offset);
    if (index < 0) {
      result.push({ text: text.slice(offset), match: false });
      break;
    }
    result.push(
      { text: text.slice(offset, index), match: false },
      { text: text.slice(index, index + q.length), match: true },
    );
    offset = index + q.length;
  }
  return result;
});
</script>
<template>
  <template v-for="(part, index) in segments" :key="index"
    ><mark v-if="part.match">{{ part.text }}</mark
    ><template v-else>{{ part.text }}</template></template
  >
</template>
<style scoped>
mark {
  background: var(--accent);
  color: var(--foreground);
  text-decoration: underline;
  text-decoration-color: var(--ring);
}
</style>
