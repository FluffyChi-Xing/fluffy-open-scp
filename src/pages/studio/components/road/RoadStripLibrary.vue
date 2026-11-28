<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  stripDragType,
  stripKinds,
  stripWidths,
  type StripKind,
} from "./roadDraft";

const emit = defineEmits<{ add: [kind: StripKind] }>();
defineProps<{ widths?: Record<StripKind, number> }>();
const { t } = useI18n();

function drag(event: DragEvent, kind: StripKind) {
  event.dataTransfer?.setData(stripDragType, kind);
}
</script>

<template>
  <section class="library" :aria-label="t('roadAsset.components.title')">
    <strong>{{ t("roadAsset.components.title") }}</strong>
    <button
      v-for="kind in stripKinds"
      :key="kind"
      type="button"
      draggable="true"
      @dragstart="drag($event, kind)"
      @click="emit('add', kind)"
    >
      <span>{{ t(`roadAsset.strips.${kind}`) }}</span>
      <small>{{
        t("roadAsset.metres", { width: widths?.[kind] ?? stripWidths[kind] })
      }}</small>
    </button>
  </section>
</template>

<style scoped>
.library {
  display: grid;
  gap: 5px;
  padding: 10px;
  font-size: 12px;
}
strong {
  margin-block-end: 3px;
}
button {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  padding: 7px 9px;
  border: 1px solid var(--border);
  background: var(--surface-elevated);
  border-radius: 4px;
  color: var(--foreground);
  cursor: grab;
  text-align: start;
}
small {
  color: var(--muted-foreground);
  white-space: nowrap;
}
</style>
