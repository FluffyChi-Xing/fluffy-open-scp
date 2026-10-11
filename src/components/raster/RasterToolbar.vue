<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { computed } from "vue";
import FSelect from "@/components/ui/FSelect.vue";
import type { Channel, RasterTool } from "@/lib/raster-editor/schema";
const tool = defineModel<RasterTool>("tool", { required: true });
const size = defineModel<number>("size", { required: true });
const color = defineModel<string>("color", { required: true });
const alpha = defineModel<number>("alpha", { required: true });
const channel = defineModel<Channel>("channel", { required: true });
const quantized = defineModel<boolean>("quantized", { required: true });
const layer = defineModel<number>("layer", { required: true });
const tolerance = defineModel<number>("tolerance", { required: true });
const parkingLength = defineModel<number>("parkingLength", { required: true });
const parkingSpacing = defineModel<number>("parkingSpacing", {
  required: true,
});
defineProps<{
  mask: boolean;
  canUndo: boolean;
  canRedo: boolean;
  lot: boolean;
}>();
const emit = defineEmits<{ undo: []; redo: [] }>();
const tools = computed<{ id: RasterTool; label: string }[]>(() => [
  { id: "brush", label: t("rasterEditor.text102") },
  { id: "eraser", label: t("rasterEditor.text103") },
  { id: "line", label: t("rasterEditor.text104") },
  { id: "curve", label: t("rasterEditor.text105") },
  { id: "rect", label: t("rasterEditor.text106") },
  { id: "fill", label: t("rasterEditor.text107") },
  { id: "picker", label: t("rasterEditor.text108") },
]);
</script>
<template>
  <div class="raster-toolbar">
    <div role="toolbar" :aria-label="t('rasterEditor.text109')" class="tools">
      <button
        v-for="entry in tools"
        :key="entry.id"
        :aria-pressed="tool === entry.id"
        @click="tool = entry.id"
      >
        {{ entry.label }}
      </button>
      <button
        v-if="lot"
        :aria-pressed="tool === 'parking'"
        @click="tool = 'parking'"
      >
        {{ t("rasterEditor.text110") }}
      </button>
      <button :disabled="!canUndo" @click="emit('undo')">
        {{ t("rasterEditor.text111") }}</button
      ><button :disabled="!canRedo" @click="emit('redo')">
        {{ t("rasterEditor.text112") }}
      </button>
    </div>
    <div class="settings">
      <label>
        {{ t("rasterEditor.text113") }}
        <input v-model.number="size" type="number" min="1" max="256"
      /></label>
      <label v-if="tool === 'fill'">
        {{ t("rasterEditor.text114") }}
        <input v-model.number="tolerance" type="number" min="0" max="510"
      /></label>
      <template v-if="tool === 'parking'"
        ><label>
          {{ t("rasterEditor.text115") }}
          <input
            v-model.number="parkingLength"
            type="number"
            min="1"
            max="256" /></label
        ><label>
          {{ t("rasterEditor.text116") }}
          <input
            v-model.number="parkingSpacing"
            type="number"
            min="1"
            max="256" /></label
      ></template>
      <label v-if="mask">
        {{ t("rasterEditor.text117") }}
        <FSelect
          :model-value="String(layer)"
          :options="[
            { value: '0', label: t('rasterEditor.text118') },
            { value: '1', label: t('rasterEditor.text119') },
            { value: '2', label: t('rasterEditor.text120') },
            { value: '3', label: t('rasterEditor.text121') },
          ]"
          @update:model-value="layer = Number($event)"
      /></label>
      <label v-if="!mask || channel !== 'rgba'"
        >RGB <input v-model="color" type="color"
      /></label>
      <label v-if="!mask || channel !== 'rgba'"
        >Alpha <input v-model.number="alpha" type="number" min="0" max="255"
      /></label>
      <label>
        {{ t("rasterEditor.text122") }}
        <FSelect
          :model-value="channel"
          :options="
            ['rgba', 'r', 'g', 'b', 'a'].map((value) => ({
              value,
              label: value.toUpperCase(),
            }))
          "
          @update:model-value="channel = $event as Channel"
      /></label>
      <label v-if="mask"
        ><input v-model="quantized" type="checkbox" />
        {{ t("rasterEditor.text123") }}
      </label>
    </div>
  </div>
</template>
<style scoped>
.raster-toolbar {
  display: grid;
  gap: 8px;
  padding: 10px;
  border-bottom: 1px solid var(--border);
}
.tools,
.settings {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px;
}
button,
select,
input {
  color: var(--foreground);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 4px;
  min-height: 28px;
  font-size: 11px;
}
button {
  padding: 4px 9px;
  cursor: pointer;
}
button[aria-pressed="true"] {
  color: var(--primary);
  border-color: var(--primary);
}
button:disabled {
  opacity: 0.4;
  cursor: default;
}
label {
  white-space: nowrap;
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--muted-foreground);
  font-size: 11px;
}
input[type="number"] {
  width: 53px;
  padding-inline: 5px;
}
input[type="color"] {
  width: 30px;
  padding: 2px;
}
:focus-visible {
  outline: 2px solid var(--primary);
}
.settings :deep(.f-select) {
  min-height: 28px;
  font-size: 11px;
  padding-inline: 7px;
}
</style>
