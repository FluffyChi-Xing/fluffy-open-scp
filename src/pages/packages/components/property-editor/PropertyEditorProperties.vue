<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { LotUnitDto, UnitKeyDto } from "@/api/tauri";

const props = defineProps<{ unit: LotUnitDto | null }>();
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
.fields-table {
  border-collapse: collapse;
  font-size: 11px;
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
.fields-table td {
  border-top: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
  color: var(--muted-foreground);
  overflow-wrap: anywhere;
  padding: 5px 6px;
  vertical-align: top;
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
