<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import LotPatternGrid from "@/components/raster/LotPatternGrid.vue";
import {
  DEFAULT_MATERIAL_COLOR,
  LOT_SLOT_COUNT,
  firstFreeSlot,
  type LotMaterial,
  type LotSlots,
} from "@/lib/lot-ground/ground";
import { atlasCellStyle } from "./lotAtlas";

const props = defineProps<{
  slots: LotSlots;
  active: number;
  baseTile: number;
  atlasPng: string | null;
  /** 绑定到已有 Lot 时只允许编辑布局，颜色和纹理保持来源值。 */
  locked?: boolean;
  disabled?: boolean;
  /** 画布上该槽是否已有绘制像素；已绘制时换纹理会新开槽。 */
  painted?: (slot: number) => boolean;
}>();
const emit = defineEmits<{
  "update:slots": [slots: LotSlots];
  "update:active": [slot: number];
  "update:baseTile": [tile: number];
  remove: [slot: number];
}>();
const { t } = useI18n();

const used = computed(() =>
  props.slots.flatMap((material, slot) => (material ? [{ slot, material }] : [])),
);
const full = computed(() => firstFreeSlot(props.slots) < 0);
const activeMaterial = computed(() => props.slots[props.active] ?? null);
const editable = computed(() => !props.disabled && !props.locked);
const activePainted = computed(() => props.painted?.(props.active) === true);

function hex(color: readonly number[]) {
  return `#${color
    .slice(0, 3)
    .map((value) => Math.round(value).toString(16).padStart(2, "0"))
    .join("")}`;
}

function parseHex(value: string): [number, number, number] {
  const hexValue = value.replace("#", "");
  return [0, 2, 4].map((at) => Number.parseInt(hexValue.slice(at, at + 2), 16) || 0) as [
    number,
    number,
    number,
  ];
}

function patch(slot: number, next: LotMaterial | null) {
  const slots = props.slots.slice(0, LOT_SLOT_COUNT);
  slots[slot] = next;
  emit("update:slots", slots);
}

function add() {
  const slot = firstFreeSlot(props.slots);
  if (slot < 0) return;
  const pattern = activeMaterial.value?.pattern ?? 0;
  patch(slot, { pattern, color: [...DEFAULT_MATERIAL_COLOR] });
  emit("update:active", slot);
}

function setColor(value: string) {
  const material = activeMaterial.value;
  if (material) patch(props.active, { ...material, color: parseHex(value) });
}

/**
 * 已在画布上画过的材质换纹理 → 新开一槽（有空位时），避免改写已绘区域。
 * 尚未落笔或槽位已满时仍改当前材质。
 */
function setPattern(pattern: number) {
  const material = activeMaterial.value;
  if (!material || material.pattern === pattern) return;
  const free = firstFreeSlot(props.slots);
  if (props.painted?.(props.active) && free >= 0) {
    patch(free, { pattern, color: [...DEFAULT_MATERIAL_COLOR] });
    emit("update:active", free);
    return;
  }
  patch(props.active, { ...material, pattern });
}
</script>

<template>
  <section class="lot-palette" :aria-label="t('rasterEditor.ground.title')">
    <header>
      <h3>{{ t("rasterEditor.ground.materials", { used: used.length, max: LOT_SLOT_COUNT }) }}</h3>
      <button
        type="button"
        class="add"
        :disabled="!editable || full"
        :title="full ? t('rasterEditor.ground.limit') : t('rasterEditor.ground.add')"
        @click="add"
      >
        + {{ t("rasterEditor.ground.add") }}
      </button>
    </header>

    <p v-if="locked" class="hint">{{ t("rasterEditor.ground.locked") }}</p>
    <p v-else-if="!used.length" class="hint">{{ t("rasterEditor.ground.empty") }}</p>
    <p v-else-if="full" class="hint">{{ t("rasterEditor.ground.limit") }}</p>

    <div
      v-if="used.length"
      class="material-list"
      role="radiogroup"
      :aria-label="t('rasterEditor.ground.materialList')"
    >
      <button
        v-for="{ slot, material } in used"
        :key="slot"
        type="button"
        role="radio"
        :aria-checked="active === slot"
        :disabled="disabled"
        @click="emit('update:active', slot)"
      >
        <span
          class="cell"
          :class="{ fallback: !atlasPng }"
          :style="atlasCellStyle(atlasPng, material.pattern)"
          aria-hidden="true"
        >
          <span class="chip" :style="{ background: hex(material.color) }" />
        </span>
        <span class="name">{{ t(`rasterEditor.patternNames.${material.pattern}`) }}</span>
      </button>
    </div>

    <div v-if="activeMaterial" class="editor">
      <label>
        <span>{{ t("rasterEditor.ground.color") }}</span>
        <input
          type="color"
          :value="hex(activeMaterial.color)"
          :disabled="!editable"
          :aria-label="t('rasterEditor.ground.color')"
          @input="setColor(($event.target as HTMLInputElement).value)"
        />
      </label>
      <span class="field-label">{{ t("rasterEditor.ground.pattern") }}</span>
      <p v-if="activePainted && !full" class="hint">{{ t("rasterEditor.ground.patternForkHint") }}</p>
      <p v-else-if="activePainted && full" class="hint">{{ t("rasterEditor.ground.patternReplaceHint") }}</p>
      <LotPatternGrid
        :model-value="activeMaterial.pattern"
        :disabled="!editable"
        :atlas-png="atlasPng"
        @update:model-value="setPattern"
      />
      <button type="button" class="remove" :disabled="!editable" @click="emit('remove', active)">
        {{ t("rasterEditor.ground.remove") }}
      </button>
    </div>

    <section class="base" :aria-label="t('rasterEditor.ground.base')">
      <header class="base-head">
        <span
          class="cell small"
          :class="{ fallback: !atlasPng }"
          :style="atlasCellStyle(atlasPng, baseTile)"
          aria-hidden="true"
        />
        <strong>{{ t("rasterEditor.ground.base") }}</strong>
        <span class="base-name">{{ t(`rasterEditor.patternNames.${baseTile}`) }}</span>
      </header>
      <p class="hint">{{ t("rasterEditor.ground.baseHint") }}</p>
      <LotPatternGrid
        :model-value="baseTile"
        :disabled="!editable"
        :atlas-png="atlasPng"
        @update:model-value="emit('update:baseTile', $event)"
      />
    </section>
  </section>
</template>

<style scoped>
.lot-palette { display: grid; gap: 8px; }
header { align-items: center; display: flex; gap: 8px; justify-content: space-between; }
h3 { font-size: 12px; font-weight: 600; margin: 0; }
.hint { color: var(--muted-foreground); font-size: 11px; line-height: 1.45; margin: 0; }
.add, .remove { background: var(--surface-elevated); border: 1px solid var(--border); border-radius: 4px; color: var(--foreground); font-size: 11px; padding: 3px 8px; }
.add:disabled, .remove:disabled { opacity: 0.5; }
.material-list { display: grid; gap: 6px; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.material-list button { align-items: center; background: var(--surface-elevated); border: 1px solid var(--border); border-radius: 4px; color: var(--muted-foreground); display: flex; flex-direction: column; font-size: 10px; gap: 3px; padding: 4px; }
.material-list button[aria-checked="true"] { border-color: var(--primary); box-shadow: 0 0 0 1px var(--primary); color: var(--foreground); }
.cell { background-repeat: no-repeat; border-radius: 3px; display: block; height: 40px; image-rendering: pixelated; position: relative; width: 100%; }
.cell.small { display: inline-block; height: 18px; vertical-align: middle; width: 18px; }
.cell.fallback { background: repeating-linear-gradient(45deg, #8d8d8d 0 3px, #cfcfcf 3px 6px); }
.chip { border: 1px solid rgb(255 255 255 / 0.8); border-radius: 50%; bottom: 3px; box-shadow: 0 0 0 1px rgb(0 0 0 / 0.35); height: 14px; position: absolute; right: 3px; width: 14px; }
.name { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.editor { border-top: 1px solid var(--border); display: grid; gap: 6px; padding-top: 8px; }
.editor label { align-items: center; display: flex; font-size: 11px; justify-content: space-between; }
.field-label { font-size: 11px; }
.base {
  border-top: 1px solid var(--border);
  display: grid;
  gap: 6px;
  padding-top: 8px;
}
.base-head {
  align-items: center;
  display: flex;
  gap: 6px;
  min-width: 0;
}
.base-head strong { flex: none; font-size: 12px; font-weight: 600; }
.base-name {
  color: var(--muted-foreground);
  flex: 1;
  font-size: 11px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
