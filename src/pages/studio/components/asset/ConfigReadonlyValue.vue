<script setup lang="ts">
import { useI18n } from "vue-i18n";
defineOptions({ name: "ConfigReadonlyValue" });
defineProps<{ value: unknown }>();
const { t, te, locale } = useI18n();
function label(key: string) {
  const aliases: Record<string, string> = {
    label: "menuName",
    enabled: "enabled",
    uiPosition: "uiPosition",
    iconPng: "iconPng",
    marqueePng: "marqueePng",
  };
  const candidates = [
    `studio.assetWorkbench.projectConfig.${key}`,
    `studio.assetWorkbench.${aliases[key] ?? key}`,
    `studio.assetWorkbench.labor.${key}`,
    `flow.variant.${key}`,
    `studio.assetWorkbench.overviewFields.${key}`,
  ];
  return candidates.find((k) => te(k))
    ? t(candidates.find((k) => te(k))!)
    : key;
}
function scalar(value: unknown) {
  if (value === null || value === undefined || value === "")
    return t("studio.assetWorkbench.overviewEmpty");
  if (typeof value === "boolean")
    return t(
      value
        ? "studio.assetWorkbench.overviewYes"
        : "studio.assetWorkbench.overviewNo",
    );
  if (typeof value === "number")
    return new Intl.NumberFormat(locale.value, {
      maximumFractionDigits: 6,
    }).format(value);
  return String(value);
}
</script>
<template>
  <dl
    v-if="value && typeof value === 'object' && !Array.isArray(value)"
    class="readonly-values"
  >
    <div v-for="(item, key) in value" :key="key" class="readonly-row">
      <dt>{{ label(String(key)) }}</dt>
      <dd><ConfigReadonlyValue :value="item" /></dd>
    </div>
    <span v-if="!Object.keys(value).length">{{
      t("studio.assetWorkbench.overviewEmpty")
    }}</span>
  </dl>
  <ol v-else-if="Array.isArray(value) && value.length" class="readonly-array">
    <li v-for="(item, index) in value" :key="index">
      <ConfigReadonlyValue :value="item" />
    </li>
  </ol>
  <span v-else>{{
    Array.isArray(value)
      ? t("studio.assetWorkbench.overviewEmpty")
      : scalar(value)
  }}</span>
</template>
<style scoped>
.readonly-values {
  margin: 0;
  display: grid;
  gap: 8px;
}
.readonly-row {
  display: grid;
  grid-template-columns: minmax(90px, 160px) minmax(0, 1fr);
  gap: 12px;
  padding: 5px 0;
}
dt {
  color: var(--muted-foreground);
  font-size: 12px;
  overflow-wrap: anywhere;
}
dd {
  margin: 0;
  min-width: 0;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
  font-size: 12px;
}
.readonly-array {
  margin: 0;
  padding-inline-start: 20px;
  display: grid;
  gap: 8px;
}
@media (max-width: 650px) {
  .readonly-row {
    grid-template-columns: 1fr;
    gap: 4px;
  }
}
</style>
