<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { computed } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FTooltip from "@/components/ui/FTooltip.vue";
import type { RasterTool } from "@/lib/raster-editor/schema";
const tool = defineModel<RasterTool>("tool", { required: true });
defineProps<{ canUndo: boolean; canRedo: boolean; lot: boolean }>();
const emit = defineEmits<{ undo: []; redo: [] }>();
const tools = computed<{ id: RasterTool; label: string; icon: string }[]>(() => [
  { id: "brush", label: t("rasterEditor.text102"), icon: "Brush" },
  { id: "eraser", label: t("rasterEditor.text103"), icon: "Eraser" },
  { id: "line", label: t("rasterEditor.text104"), icon: "Slash" },
  { id: "curve", label: t("rasterEditor.text105"), icon: "Spline" },
  { id: "rect", label: t("rasterEditor.text106"), icon: "Square" },
  { id: "rect-fill", label: t("rasterEditor.filledRectangle"), icon: "SquarePen" },
  { id: "polyline", label: t("rasterEditor.polyline"), icon: "Pencil" },
  { id: "polygon", label: t("rasterEditor.polygon"), icon: "GitBranch" },
  { id: "fill", label: t("rasterEditor.text107"), icon: "PaintBucket" },
  { id: "picker", label: t("rasterEditor.text108"), icon: "Pipette" },
]);
const hint = (entry: { id: RasterTool; label: string }) =>
  ["curve", "polyline", "polygon"].includes(entry.id)
    ? `${entry.label} · ${entry.id === "curve" ? t("rasterEditor.curveHint") : t("rasterEditor.multiPointHint")}`
    : entry.label;
</script>
<template>
  <div class="raster-toolbar">
    <div role="toolbar" :aria-label="t('rasterEditor.text109')" class="tools">
      <FTooltip v-for="entry in tools" :key="entry.id" :text="hint(entry)">
        <template #trigger>
          <button type="button" :aria-label="entry.label" :aria-pressed="tool === entry.id" @click="tool = entry.id">
            <FIcon :name="entry.icon" :size="16" />
          </button>
        </template>
      </FTooltip>
      <FTooltip v-if="lot" :text="t('rasterEditor.text110')">
        <template #trigger><button type="button" :aria-label="t('rasterEditor.text110')" :aria-pressed="tool === 'parking'" @click="tool = 'parking'"><FIcon name="SquareParking" :size="16" /></button></template>
      </FTooltip>
      <FTooltip v-if="lot" :text="t('rasterEditor.selectLayer')">
        <template #trigger><button type="button" :aria-label="t('rasterEditor.selectLayer')" :aria-pressed="tool === 'layer-picker'" @click="tool = 'layer-picker'"><FIcon name="Layers" :size="16" /></button></template>
      </FTooltip>
      <span class="toolbar-divider" aria-hidden="true" />
      <FTooltip :text="t('rasterEditor.text111')"><template #trigger><button type="button" :aria-label="t('rasterEditor.text111')" :disabled="!canUndo" @click="emit('undo')"><FIcon name="Undo2" :size="16" /></button></template></FTooltip>
      <FTooltip :text="t('rasterEditor.text112')"><template #trigger><button type="button" :aria-label="t('rasterEditor.text112')" :disabled="!canRedo" @click="emit('redo')"><FIcon name="Redo2" :size="16" /></button></template></FTooltip>
    </div>
  </div>
</template>
<style scoped>
.raster-toolbar {
  display: block;
  padding: 5px;
}
.tools {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  gap: 5px;
}
.tools {
  overflow-x: auto;
  scrollbar-width: thin;
  padding-bottom: 0;
}
button {
  color: var(--foreground);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 4px;
  min-height: 30px;
  width: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  padding: 0;
  cursor: pointer;
}
button[aria-pressed="true"] {
  color: var(--primary);
  border-color: color-mix(in srgb, var(--primary) 55%, var(--border));
  background: var(--accent);
}
button:disabled {
  opacity: 0.4;
  cursor: default;
}
.toolbar-divider {
  width: 1px;
  height: 20px;
  margin-inline: 3px;
  background: var(--border);
}
:focus-visible {
  outline: 2px solid var(--primary);
}
</style>

