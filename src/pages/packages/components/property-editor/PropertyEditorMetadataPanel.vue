<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FColorPicker from "@/components/ui/FColorPicker.vue";
import type { LightUnit, LotUnitDto, PropUnit } from "@/api/tauri";
import { unitId } from "./unitGizmos";

/**
 * 元数据 tab：按 Unit 类型给出语义字段编辑（写本地字段 override，可撤销）。
 * 覆盖 light（类型/颜色/半径/漫射/长度）与 prop（缩放倍率）；其余类型只读
 * 提示（decal 按用户裁决暂缓）。新组件族按节追加（divider 语义）。
 */
const props = defineProps<{
  unit: LotUnitDto | null;
  /** 选中 prop 是否可编辑「缩放倍率」（真实模型渲染分支才消费）。 */
  scaleEditable?: boolean;
  /** 编辑模式解锁（toolbar Lock/LockOpen）——锁定时只读展示。 */
  editEnabled?: boolean;
}>();
const emit = defineEmits<{
  "update-fields": [id: string, patch: Record<string, unknown>];
}>();
const { t } = useI18n();

const light = computed(() =>
  props.unit?.kind === "light" ? (props.unit as LightUnit) : null,
);
const prop = computed(() =>
  props.unit?.kind === "prop" ? (props.unit as PropUnit) : null,
);
/** 锁定 = toolbar 未解锁：元数据全部控件只读（解锁门控统一入口）。 */
const locked = computed(() => props.editEnabled !== true);

// prop 真实模型（树/放置直挂）：缩放倍率（DTO 独立 scale 字段，null 视为 1；
// 渲染器烘焙、提交除回，与手柄/增量路径同一语义——见 tryIncrementalGrouping）
const scaleDraft = ref("");
watch(
  () => [props.unit, props.scaleEditable] as const,
  ([unit]) => {
    scaleDraft.value =
      unit && unit.kind === "prop" && typeof unit.scale === "number"
        ? String(unit.scale)
        : "1";
  },
  { immediate: true },
);
function commitScale() {
  const unit = props.unit;
  if (locked.value || !unit || unit.kind !== "prop") return;
  const value = Number.parseFloat(scaleDraft.value);
  const current = typeof unit.scale === "number" ? unit.scale : 1;
  if (!Number.isFinite(value) || value <= 0) return;
  if (Math.abs(value - current) < 1e-6) return;
  emit("update-fields", unitId(unit), { scale: value });
}

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
  if (locked.value || !light.value) return;
  emit("update-fields", unitId(light.value), { lightType: type });
  typeOpen.value = false;
}

function setNumberField(key: NumberField["key"], event: Event) {
  if (locked.value || !light.value) return;
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
  if (locked.value || !light.value) return;
  emit("update-fields", unitId(light.value), { color: hexToFloats(hex) });
}
</script>

<template>
  <div class="metadata-panel">
    <p v-if="!unit" class="panel-empty">{{ $t("package.noUnitSelected") }}</p>
    <template v-else-if="prop">
      <!-- prop 缩放倍率（从属性 tab 移入元数据 tab，2026-10-08 用户指令）：
           DTO 独立 scale 字段，null 视为 1；真实模型渲染分支消费 -->
      <!-- 布局（2026-10-08 用户指令）：左侧上下=名称+解释，右侧=输入框；
           仅 toolbar 解锁（editEnabled）后可编辑 -->
      <section class="panel-section">
        <div class="scale-row">
          <div class="scale-label">
            <span>{{ $t("package.scaleMultiplier") }}</span>
            <small>{{ $t("package.scaleMultiplierHint") }}</small>
          </div>
          <input
            v-model="scaleDraft"
            class="scale-input"
            type="number"
            step="0.01"
            min="0.01"
            :disabled="locked"
            :title="locked ? $t('package.editUnlockFirst') : undefined"
            :aria-label="$t('package.scaleMultiplier')"
            @change="commitScale"
          />
        </div>
      </section>
    </template>
    <template v-else-if="light">
      <!-- 灯光三节：统一左标签/右控件布局；锁定（toolbar 未解锁）时全部禁用 -->
      <section class="panel-section">
        <div class="scale-row">
          <div class="scale-label">
            <span>{{ $t("package.lightType") }}</span>
          </div>
          <FDropdown v-model:open="typeOpen" :width="150">
            <template #trigger>
              <button
                type="button"
                class="type-trigger"
                :disabled="locked"
              >
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
        </div>
      </section>
      <section class="panel-section">
        <div class="scale-row">
          <div class="scale-label">
            <span>{{ $t("package.lightColor") }}</span>
          </div>
          <div class="color-control" :class="{ locked }">
            <FColorPicker
              :model-value="colorHex"
              :title="t('common.colorPicker')"
              @update:model-value="setColor"
            />
          </div>
        </div>
        <p class="readonly-hex">{{ colorHex }}</p>
      </section>
      <section class="panel-section">
        <div class="axis-rows">
          <label v-for="field in NUMBER_FIELDS" :key="field.key" class="axis-row">
            <span class="field-label">{{ t(field.label) }}</span>
            <input
              type="number"
              :step="field.step"
              :value="light[field.key] ?? 0"
              :disabled="locked"
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
  white-space: nowrap;
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
.field-hint {
  color: var(--subtle-foreground);
  font-size: 10px;
  margin: 2px 0 0;
}
/* 缩放倍率行：左=名称+解释（上下堆叠），右=输入框 */
.scale-row {
  align-items: center;
  display: flex;
  gap: 10px;
  justify-content: space-between;
}
.scale-label {
  display: flex;
  flex-direction: column;
}
.scale-label > span {
  color: var(--foreground);
  font-size: 12px;
}
.scale-label > small {
  color: var(--subtle-foreground);
  font-size: 10px;
}
.scale-input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  flex: none;
  font: inherit;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  min-height: 26px;
  padding: 0 8px;
  text-align: end;
  width: 96px;
}
.scale-input:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.metadata-panel.locked .type-trigger,
.metadata-panel.locked .axis-row input,
.metadata-panel.locked .scale-input {
  cursor: not-allowed;
  opacity: 0.55;
}
.metadata-panel.locked .color-control {
  cursor: not-allowed;
  opacity: 0.55;
  pointer-events: none;
}
/* 颜色选择器触发器 width:100%——容器须占满行内剩余宽度，否则内缩。
   FPopover 锚点是 inline-flex（内容定宽），需转为 flex item 并让其
   撑满，按钮的 width:100% 才有正确的百分比基准。 */
.scale-row .color-control {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
}
.scale-row .color-control :deep(.f-popover-anchor) {
  flex: 1 1 auto;
  width: auto;
}
.panel-empty {
  color: var(--subtle-foreground);
  font-size: 12px;
  margin: 12px;
}
</style>
