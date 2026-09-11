<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FColorPicker from "@/components/ui/FColorPicker.vue";
import type { LightUnit, LotUnitDto } from "@/api/tauri";
import { unitId } from "./unitGizmos";

/**
 * 元数据 tab：按 Unit 类型给出语义字段编辑（写本地字段 override，可撤销）。
 * MVP 覆盖 light（类型/颜色/半径/漫射/长度）；其余类型只读提示
 * （decal 按用户裁决暂缓）。
 */
const props = defineProps<{ unit: LotUnitDto | null }>();
const emit = defineEmits<{
  "update-fields": [id: string, patch: Record<string, unknown>];
}>();
const { t } = useI18n();

const light = computed(() =>
  props.unit?.kind === "light" ? (props.unit as LightUnit) : null,
);

const LIGHT_TYPES = ["Point", "Spot", "Line"] as const;
const typeOpen = ref(false);

interface NumberField {
  key: "outerRadius" | "innerRadius" | "diffuse" | "length";
  label: string;
  step: number;
}
const NUMBER_FIELDS: NumberField[] = [
  { key: "outerRadius", label: "package.lightRadius", step: 0.5 },
  { key: "innerRadius", label: "package.lightInnerRadius", step: 0.5 },
  { key: "diffuse", label: "package.lightDiffuse", step: 0.05 },
  { key: "length", label: "package.lightLength", step: 0.5 },
];

function setLightType(type: string) {
  if (!light.value) return;
  emit("update-fields", unitId(light.value), { lightType: type });
  typeOpen.value = false;
}

function setNumberField(key: NumberField["key"], event: Event) {
  if (!light.value) return;
  const value = Number((event.target as HTMLInputElement).value);
  if (!Number.isFinite(value)) return;
  emit("update-fields", unitId(light.value), { [key]: value });
}

/** 0-1 浮点三元组 ↔ "#rrggbb"。 */
function floatsToHex(color: [number, number, number] | null): string {
  const channels = color ?? [1, 1, 1];
  return `#${channels
    .map((channel) =>
      Math.round(Math.min(Math.max(channel, 0), 1) * 255)
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
}
function hexToFloats(hex: string): [number, number, number] {
  return [1, 3, 5].map((offset) => parseInt(hex.slice(offset, offset + 2), 16) / 255) as [
    number,
    number,
    number,
  ];
}
const colorHex = computed(() => floatsToHex(light.value?.color ?? null));
function setColor(hex: string) {
  if (!light.value) return;
  emit("update-fields", unitId(light.value), { color: hexToFloats(hex) });
}
</script>

<template>
  <div class="metadata-panel">
    <p v-if="!unit" class="panel-empty">{{ $t("package.noUnitSelected") }}</p>
    <template v-else-if="light">
      <section class="panel-section">
        <h4>{{ $t("package.lightType") }}</h4>
        <FDropdown v-model:open="typeOpen" :width="160">
          <template #trigger>
            <button type="button" class="type-trigger">
              <span>{{ light.lightType ?? "Point" }}</span>
              <FIcon name="ChevronDown" :size="14" aria-label="" />
            </button>
          </template>
          <button
            v-for="type in LIGHT_TYPES"
            :key="type"
            type="button"
            :class="{ selected: (light.lightType ?? 'Point') === type }"
            @click="setLightType(type)"
          >
            <FIcon
              :name="(light.lightType ?? 'Point') === type ? 'Check' : 'CircleDot'"
              :size="14"
              aria-label=""
            />
            {{ type }}
          </button>
        </FDropdown>
      </section>
      <section class="panel-section">
        <h4>{{ $t("package.lightColor") }}</h4>
        <div class="axis-row color-row">
          <span class="field-label">{{ t("package.lightColor") }}</span>
          <FColorPicker
            :model-value="colorHex"
            :title="t('common.colorPicker')"
            @update:model-value="setColor"
          />
        </div>
        <p class="readonly-hex">{{ colorHex }}</p>
      </section>
      <section class="panel-section">
        <h4>{{ $t("package.metadataNumbers") }}</h4>
        <div class="axis-rows">
          <label v-for="field in NUMBER_FIELDS" :key="field.key" class="axis-row">
            <span class="field-label">{{ t(field.label) }}</span>
            <input
              type="number"
              :step="field.step"
              :value="light[field.key] ?? 0"
              :aria-label="t(field.label)"
              @change="setNumberField(field.key, $event)"
            />
          </label>
        </div>
      </section>
    </template>
    <p v-else class="panel-empty">{{ $t("package.metadataUnsupported") }}</p>
  </div>
</template>

<style scoped>
.metadata-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
}
.panel-section h4 {
  color: var(--subtle-foreground);
  font-size: 11px;
  font-weight: 600;
  margin: 0 0 6px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.type-trigger {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  gap: 8px;
  justify-content: space-between;
  min-height: 28px;
  padding: 0 8px;
  width: 100%;
}
.axis-rows {
  display: grid;
  gap: 4px;
}
.axis-row {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: 56px 1fr;
}
.color-row {
  min-height: 28px;
}
.field-label {
  color: var(--muted-foreground);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.readonly-hex {
  color: var(--subtle-foreground);
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
  margin: 4px 0 0 64px;
}
.axis-row input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  min-height: 26px;
  padding: 0 8px;
  width: 100%;
}
.panel-empty {
  color: var(--subtle-foreground);
  font-size: 12px;
  margin: 12px;
}
</style>
