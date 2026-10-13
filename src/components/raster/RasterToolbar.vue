<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { computed } from "vue";
import FSelect from "@/components/ui/FSelect.vue";
import { Switch } from "@/components/ui/switch";
import type {
  Channel,
  RasterTool,
  RasterProfile,
} from "@/lib/raster-editor/schema";
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
const props = withDefaults(
  defineProps<{
    profile?: RasterProfile;
    mask: boolean;
    canUndo: boolean;
    canRedo: boolean;
    lot: boolean;
  }>(),
  { profile: "rgba" },
);
const channelValue = computed({
  get: () => {
    if (channel.value === "a") return alpha.value;
    const offset = { r: 1, g: 3, b: 5, rgba: 1 }[channel.value];
    return parseInt(color.value.slice(offset, offset + 2), 16);
  },
  set: (value: number) => {
    const byte = Math.max(0, Math.min(255, Math.round(value || 0)));
    if (channel.value === "a") alpha.value = byte;
    else color.value = "#" + byte.toString(16).padStart(2, "0").repeat(3);
  },
});
const emit = defineEmits<{ undo: []; redo: [] }>();
const tools = computed<{ id: RasterTool; label: string }[]>(() => [
  { id: "brush", label: t("rasterEditor.text102") },
  { id: "eraser", label: t("rasterEditor.text103") },
  { id: "line", label: t("rasterEditor.text104") },
  { id: "curve", label: t("rasterEditor.text105") },
  { id: "rect", label: t("rasterEditor.text106") },
  { id: "rect-fill", label: t("rasterEditor.filledRectangle") },
  { id: "polyline", label: t("rasterEditor.polyline") },
  { id: "polygon", label: t("rasterEditor.polygon") },
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
        :title="entry.id === 'curve' ? t('rasterEditor.curveHint') : ['polyline', 'polygon'].includes(entry.id) ? t('rasterEditor.multiPointHint') : entry.label"
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
      <button v-if="lot && quantized" :aria-pressed="tool === 'layer-picker'" @click="tool = 'layer-picker'">
        {{ t("rasterEditor.selectLayer") }}
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
            {
              value: '0',
              label: lot
                ? t('rasterEditor.layer4')
                : t('rasterEditor.decalLayer', { n: 1, channel: 'R' }),
            },
            {
              value: '1',
              label: lot
                ? t('rasterEditor.layer3')
                : t('rasterEditor.decalLayer', { n: 2, channel: 'G' }),
            },
            {
              value: '2',
              label: lot
                ? t('rasterEditor.layer2')
                : t('rasterEditor.decalLayer', { n: 3, channel: 'B' }),
            },
            {
              value: '3',
              label: lot
                ? t('rasterEditor.layer1')
                : t('rasterEditor.decalLayer', { n: 4, channel: 'A' }),
            },
          ]"
          @update:model-value="layer = Number($event)"
      /></label>
      <label v-if="!mask && channel === 'rgba'"
        >RGB <input v-model="color" type="color"
      /></label>
      <label
        v-if="channel === 'rgba' && (!mask || profile === 'decal-graffiti')"
        >{{ t("rasterEditor.channelStrength") }}
        <input v-model.number="alpha" type="number" min="0" max="255"
      /></label>
      <label v-if="channel !== 'rgba'"
        >{{
          t("rasterEditor.channelValue", { channel: channel.toUpperCase() })
        }}
        <input v-model.number="channelValue" type="number" min="0" max="255" />
      </label>
      <label>
        {{ t("rasterEditor.text122") }}
        <FSelect
          :model-value="channel"
          :options="
            ['rgba', 'r', 'g', 'b', 'a'].map((value) => ({
              value,
              label: lot
                ? t(`rasterEditor.channels.${value}`)
                : value.toUpperCase(),
            }))
          "
          @update:model-value="channel = $event as Channel"
      /></label>
      <label v-if="lot"
        ><Switch v-model="quantized" :aria-label="t('rasterEditor.text123')" />
        {{ t("rasterEditor.text123") }}
      </label>
    </div>
    <p v-if="lot" class="lot-guide">{{ t("rasterEditor.lotGuide") }}</p>
    <p class="lot-guide">
      {{ t(`rasterEditor.profileGuide.${props.profile}`) }}
    </p>
  </div>
</template>
<style scoped>
.lot-guide {
  margin: 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--muted-foreground);
}
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

