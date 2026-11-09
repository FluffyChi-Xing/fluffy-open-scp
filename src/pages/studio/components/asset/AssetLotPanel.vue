<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  rotationFromStreetFacing,
  streetFacingFromRotation,
  type StreetFacingDegrees,
} from "./streetFacing";

/**
 * 右栏「地块」页签：draft 模式下编辑 footprint；四向临街朝向选择
 * （盒模型式四边按钮，默认 +Y 高亮；切换实时驱动 viewport 箭头与模型 yaw）。
 */
const props = defineProps<{
  lotSize: [number, number] | null;
  editMode: "native" | "draft";
  rotationDegrees: StreetFacingDegrees;
}>();
const emit = defineEmits<{
  "set-size": [size: [number, number]];
  "set-rotation": [degrees: StreetFacingDegrees];
}>();

const { t } = useI18n();
const width = computed({
  get: () => props.lotSize?.[0] ?? 0,
  set: (value: number) => {
    if (!props.lotSize || props.editMode !== "draft") return;
    emit("set-size", [value, props.lotSize[1]]);
  },
});
const depth = computed({
  get: () => props.lotSize?.[1] ?? 0,
  set: (value: number) => {
    if (!props.lotSize || props.editMode !== "draft") return;
    emit("set-size", [props.lotSize[0], value]);
  },
});

const activeFacing = computed(() =>
  streetFacingFromRotation(props.rotationDegrees),
);

function selectFacing(facing: StreetFacingDegrees) {
  emit("set-rotation", rotationFromStreetFacing(facing));
}

const edges: {
  facing: StreetFacingDegrees;
  edge: "posY" | "negX" | "negY" | "posX";
  className: string;
}[] = [
  { facing: 0, edge: "posY", className: "edge-top" },
  { facing: 90, edge: "negX", className: "edge-left" },
  { facing: 180, edge: "negY", className: "edge-bottom" },
  { facing: 270, edge: "posX", className: "edge-right" },
];
</script>

<template>
  <div class="lot-panel" :aria-label="t('studio.assetWorkbench.lotTab')">
    <template v-if="lotSize">
      <div class="lot-size-grid">
        <label class="lot-field">
          <span>{{ t("studio.assetWorkbench.lotWidth") }}</span>
          <input
            v-model.number="width"
            type="number"
            min="4"
            :disabled="editMode !== 'draft'"
          />
        </label>
        <label class="lot-field">
          <span>{{ t("studio.assetWorkbench.lotDepth") }}</span>
          <input
            v-model.number="depth"
            type="number"
            min="4"
            :disabled="editMode !== 'draft'"
          />
        </label>
      </div>

      <section
        class="facing-card"
        :aria-label="t('studio.assetWorkbench.streetFacingTitle')"
      >
        <header class="facing-head">
          <strong>{{ t("studio.assetWorkbench.streetFacingTitle") }}</strong>
          <span class="facing-deg">{{ rotationDegrees }}°</span>
        </header>
        <p class="facing-hint">
          {{ t("studio.assetWorkbench.streetFacingHint") }}
        </p>
        <div class="facing-box" role="radiogroup">
          <button
            v-for="item in edges"
            :key="item.edge"
            type="button"
            class="facing-edge"
            :class="[item.className, { active: activeFacing === item.facing }]"
            role="radio"
            :aria-checked="activeFacing === item.facing"
            :aria-label="
              t(`studio.assetWorkbench.streetFacingEdges.${item.edge}`)
            "
            @click="selectFacing(item.facing)"
          >
            <span class="facing-arrow" aria-hidden="true" />
            <span class="facing-label">{{
              t(`studio.assetWorkbench.streetFacingEdges.${item.edge}`)
            }}</span>
          </button>
          <div class="facing-core" aria-hidden="true">
            <span>{{ t("studio.assetWorkbench.streetFacingLot") }}</span>
          </div>
        </div>
      </section>

      <p v-if="editMode === 'draft'" class="lot-hint">
        {{ t("studio.assetWorkbench.lotDraftHint") }}
      </p>
      <p v-else class="lot-hint">
        {{ t("studio.assetWorkbench.lotNativeHint") }}
      </p>
    </template>
    <p v-else class="lot-hint">{{ t("studio.assetWorkbench.lotEmpty") }}</p>
  </div>
</template>

<style scoped>
.lot-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
}
.lot-size-grid {
  display: grid;
  gap: 8px;
  grid-template-columns: 1fr 1fr;
}
.lot-field {
  color: var(--muted-foreground);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 4px;
}
.lot-field input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  min-height: 28px;
  padding: 0 8px;
}
.lot-field input:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.facing-card {
  background: color-mix(in srgb, var(--surface-elevated) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-md, 10px);
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
}
.facing-head {
  align-items: center;
  display: flex;
  justify-content: space-between;
}
.facing-head strong {
  font-size: 12px;
  font-weight: 600;
}
.facing-deg {
  background: color-mix(in srgb, var(--primary, #2563eb) 16%, transparent);
  border-radius: 999px;
  color: var(--primary, #60a5fa);
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  padding: 2px 7px;
}
.facing-hint {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}
.facing-box {
  aspect-ratio: 1;
  display: grid;
  gap: 3px;
  grid-template-areas:
    ". top ."
    "left core right"
    ". bottom .";
  grid-template-columns: 28px 1fr 28px;
  grid-template-rows: 28px 1fr 28px;
  margin: 2px auto 0;
  max-width: 220px;
  width: 100%;
}
.facing-edge {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 70%, transparent);
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  gap: 4px;
  justify-content: center;
  padding: 0;
  transition:
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease;
}
.facing-edge:hover {
  border-color: color-mix(in srgb, var(--primary, #2563eb) 45%, var(--border));
  color: var(--foreground);
}
.facing-edge.active {
  background: color-mix(in srgb, #ff8a00 22%, var(--surface-elevated));
  border-color: #ff8a00;
  color: #ffb35c;
  box-shadow: inset 0 0 0 1px color-mix(in srgb, #ff8a00 35%, transparent);
}
.edge-top {
  border-radius: 8px 8px 4px 4px;
  flex-direction: column;
  grid-area: top;
}
.edge-bottom {
  border-radius: 4px 4px 8px 8px;
  flex-direction: column-reverse;
  grid-area: bottom;
}
.edge-left {
  border-radius: 8px 4px 4px 8px;
  flex-direction: row;
  grid-area: left;
  writing-mode: vertical-rl;
}
.edge-right {
  border-radius: 4px 8px 8px 4px;
  flex-direction: row-reverse;
  grid-area: right;
  writing-mode: vertical-rl;
}
.facing-arrow {
  background: currentColor;
  clip-path: polygon(50% 0, 100% 100%, 0 100%);
  flex-shrink: 0;
  height: 7px;
  width: 8px;
}
.edge-bottom .facing-arrow {
  transform: rotate(180deg);
}
.edge-left .facing-arrow {
  transform: rotate(-90deg);
}
.edge-right .facing-arrow {
  transform: rotate(90deg);
}
.facing-label {
  font-size: 9px;
  letter-spacing: 0.02em;
  line-height: 1;
  opacity: 0.9;
}
.facing-core {
  align-items: center;
  background: color-mix(in srgb, var(--surface-elevated) 55%, #1a2333);
  border: 1px dashed color-mix(in srgb, var(--border) 80%, transparent);
  border-radius: 6px;
  color: var(--subtle-foreground);
  display: flex;
  font-size: 10px;
  grid-area: core;
  justify-content: center;
}
.lot-hint {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
}
</style>
