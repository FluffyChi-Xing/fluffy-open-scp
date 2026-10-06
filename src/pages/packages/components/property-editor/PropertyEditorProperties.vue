<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { LotUnitDto, UnitKeyDto } from "@/api/tauri";
import { unitId } from "./unitGizmos";

const props = defineProps<{
  unit: LotUnitDto | null;
  /** 选中 prop 是否显示「缩放倍率」编辑（真实模型渲染分支才消费）。 */
  scaleEditable?: boolean;
}>();
const emit = defineEmits<{
  "update-fields": [id: string, patch: Record<string, unknown>];
}>();
const { t } = useI18n();

interface SummaryRow {
  label: string;
  value: string;
  swatch?: string;
}

function formatKey(key: UnitKeyDto) {
  const hex = (value: number) => value.toString(16).padStart(8, "0").toUpperCase();
  return `T ${hex(key.typeId)} · G ${hex(key.group)} · I ${hex(key.instance)}`;
}
function formatVector(values: readonly number[]) {
  return `(${values.map((value) => value.toFixed(3)).join(", ")})`;
}

const summary = computed<SummaryRow[]>(() => {
  const unit = props.unit;
  if (!unit) return [];
  const rows: SummaryRow[] = [];
  switch (unit.kind) {
    case "light": {
      if (unit.lightType)
        rows.push({ label: t("package.lightType"), value: unit.lightType });
      if (unit.color)
        rows.push({
          label: t("package.lightColor"),
          value: formatVector(unit.color),
          swatch: unit.color
            .map((channel) =>
              Math.round(Math.min(Math.max(channel, 0), 1) * 255)
                .toString(16)
                .padStart(2, "0"),
            )
            .join(""),
        });
      if (unit.outerRadius !== null)
        rows.push({ label: t("package.lightRadius"), value: String(unit.outerRadius) });
      if (unit.innerRadius !== null)
        rows.push({ label: t("package.lightInnerRadius"), value: String(unit.innerRadius) });
      if (unit.diffuse !== null)
        rows.push({ label: t("package.lightDiffuse"), value: String(unit.diffuse) });
      if (unit.length !== null)
        rows.push({ label: t("package.lightLength"), value: String(unit.length) });
      if (unit.cullDistance)
        rows.push({ label: t("package.lightCullDistance"), value: unit.cullDistance });
      if (unit.isVolumetric !== null)
        rows.push({ label: t("package.lightVolumetric"), value: String(unit.isVolumetric) });
      if (unit.debugName)
        rows.push({ label: t("package.lightDebugName"), value: unit.debugName });
      break;
    }
    case "effect": {
      if (unit.effectId)
        rows.push({ label: t("package.effectId"), value: formatKey(unit.effectId) });
      if (unit.enabled !== null)
        rows.push({ label: t("package.effectEnabled"), value: String(unit.enabled) });
      break;
    }
    case "decal": {
      rows.push({ label: t("package.decalCategory"), value: String(unit.category + 1) });
      if (unit.scale !== null)
        rows.push({ label: t("package.decalScale"), value: String(unit.scale) });
      if (unit.depth !== null)
        rows.push({ label: t("package.decalDepth"), value: String(unit.depth) });
      if (unit.materialData)
        rows.push({ label: t("package.decalMaterial"), value: formatVector(unit.materialData) });
      break;
    }
    case "prop": {
      rows.push({ label: t("package.propBin"), value: String(unit.bin) });
      if (unit.slot !== null)
        rows.push({ label: t("package.propSlot"), value: String(unit.slot) });
      if (unit.scale !== null)
        rows.push({ label: t("package.propScale"), value: String(unit.scale) });
      break;
    }
    case "pathPoint": {
      if (unit.point)
        rows.push({ label: t("package.pathPointPosition"), value: formatVector(unit.point) });
      if (unit.tangent)
        rows.push({ label: t("package.pathPointTangent"), value: formatVector(unit.tangent) });
      if (unit.pointIndex !== null)
        rows.push({ label: t("package.pathPointIndex"), value: String(unit.pointIndex) });
      break;
    }
    case "spawner": {
      if (unit.id) rows.push({ label: t("package.spawnerId"), value: formatKey(unit.id) });
      if (unit.count !== null)
        rows.push({ label: t("package.spawnerCount"), value: String(unit.count) });
      if (unit.countRandom !== null)
        rows.push({ label: t("package.spawnerCountRandom"), value: String(unit.countRandom) });
      if (unit.agent)
        rows.push({ label: t("package.spawnerAgent"), value: formatKey(unit.agent) });
      break;
    }
  }
  if ("transform" in unit && unit.transform)
    rows.push({
      label: t("package.unitTransform"),
      value: formatVector(unit.transform.matrix.slice(9, 12)),
    });
  return rows;
});

const fields = computed(() => props.unit?.fields ?? []);
const empty = computed(() => !props.unit);

// ---- 定制元数据节（divider 分节）：选中组件 kind 专属的可编辑字段 ----
// 现阶段 prop 真模型（树/放置直挂）= 缩放倍率（DTO 独立 scale 字段，
// null 视为 1）；未来按组件族放开编辑，沿用本节结构逐节追加。
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
  if (!unit || unit.kind !== "prop") return;
  const value = Number.parseFloat(scaleDraft.value);
  const current = typeof unit.scale === "number" ? unit.scale : 1;
  if (!Number.isFinite(value) || value <= 0) return;
  if (Math.abs(value - current) < 1e-6) return;
  emit("update-fields", unitId(unit), { scale: value });
}
</script>

<template>
  <aside class="properties" :aria-label="$t('package.properties')">
    <template v-if="empty">
      <p class="properties-empty">{{ $t("package.noUnitSelected") }}</p>
    </template>
    <template v-else>
      <h3 class="properties-title">{{ $t("package.properties") }}</h3>
      <dl class="summary-list">
        <div v-for="row in summary" :key="row.label">
          <dt>{{ row.label }}</dt>
          <dd>
            <span
              v-if="row.swatch"
              class="swatch"
              :style="{ background: `#${row.swatch}` }"
              aria-hidden="true"
            />
            {{ row.value }}
          </dd>
        </div>
      </dl>
      <!-- 定制元数据节：divider 分节，选中不同组件族在此追加专属编辑项 -->
      <h4 class="section-title">{{ $t("package.customMetadata") }}</h4>
      <dl
        v-if="unit && scaleEditable && unit.kind === 'prop'"
        class="summary-list"
      >
        <div>
          <dt>
            {{ $t("package.scaleMultiplier") }}
            <small class="meta-hint">{{
              $t("package.scaleMultiplierHint")
            }}</small>
          </dt>
          <dd>
            <input
              v-model="scaleDraft"
              class="meta-number"
              type="number"
              step="0.01"
              min="0.01"
              :aria-label="$t('package.scaleMultiplier')"
              @change="commitScale"
            />
          </dd>
        </div>
      </dl>
      <p v-else class="meta-empty">{{ $t("package.noCustomMeta") }}</p>
      <h4 v-if="fields.length" class="fields-title">
        {{ $t("package.unitFields") }}
      </h4>
      <table v-if="fields.length" class="fields-table">
        <thead>
          <tr>
            <th>{{ $t("package.propertyColumn") }}</th>
            <th>{{ $t("package.valueColumn") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="field in fields" :key="field.hash">
            <td>
              <span>0x{{ field.hash.toString(16).padStart(8, "0").toUpperCase() }}</span>
              <small>{{ field.typeName }}</small>
            </td>
            <td>{{ field.value }}</td>
          </tr>
        </tbody>
      </table>
    </template>
  </aside>
</template>

<style scoped>
.properties {
  border-inline-start: 1px solid var(--border);
  min-height: 0;
  overflow-y: auto;
  padding: 12px;
}
.properties-empty {
  color: var(--subtle-foreground);
  font-size: 12px;
  text-align: center;
  padding-top: 32px;
}
.properties-title {
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  margin: 0 0 8px;
  text-transform: uppercase;
}
.summary-list {
  display: grid;
  gap: 7px;
  margin: 0;
}
/* 键值两列布局：键列按内容自适应并给足下限，值列占剩余宽度 */
.summary-list > div {
  align-items: baseline;
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(96px, max-content) minmax(0, 1fr);
}
.summary-list dt {
  color: var(--subtle-foreground);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
.summary-list dd {
  align-items: center;
  color: var(--foreground);
  display: flex;
  font-size: 12px;
  gap: 6px;
  margin: 1px 0 0;
  overflow-wrap: anywhere;
}
.swatch {
  border: 1px solid var(--border);
  border-radius: 3px;
  display: inline-block;
  flex: none;
  height: 12px;
  width: 12px;
}
.fields-title {
  border-top: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  margin: 14px 0 8px;
  padding-top: 10px;
  text-transform: uppercase;
}
/* 定制元数据节标题 = divider（与 fields-title 同款分隔线语义） */
.section-title {
  border-top: 1px solid var(--brand);
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  margin: 14px 0 8px;
  padding-top: 10px;
  text-transform: uppercase;
}
.meta-hint {
  color: var(--subtle-foreground);
  display: block;
  font-size: 9px;
  font-weight: 400;
  letter-spacing: 0.02em;
  margin-top: 2px;
  text-transform: none;
}
.meta-number {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  max-width: 96px;
  padding: 3px 6px;
  width: 100%;
}
.meta-number:focus {
  border-color: var(--brand);
  outline: none;
}
.meta-empty {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 0;
}
.fields-table {
  border-collapse: collapse;
  font-size: 11px;
  /* 固定布局 + 键列定宽：值列长串（Transform 12 元组等）不再挤压键列 */
  table-layout: fixed;
  width: 100%;
}
.fields-table th {
  border-bottom: 1px solid var(--border);
  color: var(--subtle-foreground);
  font-size: 9px;
  letter-spacing: 0.06em;
  padding: 4px 6px;
  text-align: start;
  text-transform: uppercase;
}
.fields-table th:first-child,
.fields-table td:first-child {
  width: 96px;
}
.fields-table td {
  border-top: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
  color: var(--muted-foreground);
  overflow-wrap: anywhere;
  padding: 5px 6px;
  vertical-align: top;
}
.fields-table td:first-child {
  overflow-wrap: normal;
}
.fields-table td span {
  color: var(--foreground);
  display: block;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.fields-table td small {
  color: var(--subtle-foreground);
  font-size: 9px;
}
</style>
