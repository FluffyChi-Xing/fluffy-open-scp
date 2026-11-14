<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  STREET_FACING_OPTIONS,
  type StreetFacingDegrees,
} from "./streetFacing";

/**
 * 右栏「地块」页签：draft 模式下编辑 footprint；四向临街吸附边选择
 * （只移动橙色箭头，不旋转建筑）。
 */
const props = defineProps<{
  lotSize: [number, number] | null;
  editMode: "native" | "draft";
  streetFacingDegrees: StreetFacingDegrees;
}>();
const emit = defineEmits<{
  "set-size": [size: [number, number]];
  "set-street-facing": [degrees: StreetFacingDegrees];
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

const activeFacing = computed(() => props.streetFacingDegrees);

function selectFacing(facing: StreetFacingDegrees) {
  emit("set-street-facing", facing);
}

const edges = STREET_FACING_OPTIONS.map((item) => ({
  ...item,
  className:
    item.edge === "posY"
      ? "edge-top"
      : item.edge === "negX"
        ? "edge-left"
        : item.edge === "negY"
          ? "edge-bottom"
          : "edge-right",
}));
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
          <span class="facing-deg">{{
            t(
              `studio.assetWorkbench.streetFacingEdges.${
                edges.find((item) => item.facing === activeFacing)?.edge ??
                "posY"
              }`,
            )
          }}</span>
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
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--foreground);
  font-size: 12px;
  padding: 6px 8px;
}
.lot-hint {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}
.facing-card {
  border: 1px solid var(--border);
  border-radius: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
}
.facing-head {
  align-items: baseline;
  display: flex;
  justify-content: space-between;
  gap: 8px;
}
.facing-head strong {
  font-size: 12px;
  font-weight: 600;
}
.facing-deg {
  color: var(--muted-foreground);
  font-size: 11px;
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
  gap: 6px;
  grid-template-areas:
    ". top ."
    "left core right"
    ". bottom .";
  grid-template-columns: 1fr 1.4fr 1fr;
  grid-template-rows: 1fr 1.4fr 1fr;
  margin: 0 auto;
  max-width: 220px;
  width: 100%;
}
.facing-edge {
  align-items: center;
  background: color-mix(in srgb, var(--muted) 55%, transparent);
  border: 1px solid var(--border);
  border-radius: 8px;
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 4px;
  justify-content: center;
  padding: 6px;
}
.facing-edge.active {
  background: color-mix(in srgb, #f59e0b 22%, var(--background));
  border-color: #f59e0b;
  color: var(--foreground);
}
.facing-edge:hover {
  border-color: color-mix(in srgb, #f59e0b 60%, var(--border));
}
.edge-top {
  grid-area: top;
}
.edge-left {
  grid-area: left;
}
.edge-right {
  grid-area: right;
}
.edge-bottom {
  grid-area: bottom;
}
.facing-arrow {
  border-left: 5px solid transparent;
  border-right: 5px solid transparent;
  border-bottom: 8px solid currentColor;
  display: block;
  height: 0;
  width: 0;
}
.edge-left .facing-arrow {
  transform: rotate(-90deg);
}
.edge-right .facing-arrow {
  transform: rotate(90deg);
}
.edge-bottom .facing-arrow {
  transform: rotate(180deg);
}
.facing-label {
  font-size: 10px;
  line-height: 1.2;
}
.facing-core {
  align-items: center;
  background: color-mix(in srgb, var(--muted) 35%, transparent);
  border: 1px dashed var(--border);
  border-radius: 8px;
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  grid-area: core;
  justify-content: center;
}
</style>
