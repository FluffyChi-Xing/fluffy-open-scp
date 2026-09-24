<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ErzPreview } from "@/api/tauri";

const props = defineProps<{ preview: ErzPreview }>();
const { t } = useI18n();

const data = computed(() => props.preview.data);

function hex32(value: number) {
  return `0x${value.toString(16).padStart(8, "0").toUpperCase()}`;
}

function num(value: number) {
  return value.toLocaleString();
}

const stats = computed(() => [
  {
    label: t("package.erz.statRules"),
    value: num(data.value.ruleCount),
    hint: t("package.erz.statRulesHint"),
  },
  {
    label: t("package.erz.statSectionA"),
    value: num(data.value.recACount),
    hint: t("package.erz.statSectionAHint"),
  },
  {
    label: t("package.erz.statSectionB"),
    value: num(data.value.recBCount),
    hint: t("package.erz.statSectionBHint"),
  },
  {
    label: t("package.erz.statSectionC"),
    value: num(data.value.recCCount),
    hint: t("package.erz.statSectionCHint"),
  },
  {
    label: t("package.erz.statConstants"),
    value: num(data.value.constantCount),
    hint: t("package.erz.statConstantsHint"),
  },
  {
    label: t("package.erz.statDSection"),
    value: `${num(data.value.dObjects)} / ${num(data.value.dEntries)}`,
    hint: t("package.erz.statDSectionHint"),
  },
  {
    label: t("package.erz.statESection"),
    value: `${num(data.value.eObjects)} / ${num(data.value.eEntries)}`,
    hint: t("package.erz.statESectionHint"),
  },
  {
    label: t("package.erz.statPool"),
    value: `${num(data.value.blobLength)} B`,
    hint: t("package.erz.statPoolHint"),
  },
]);

const shownConstants = computed(() => data.value.constants.slice(0, 64));
const hiddenConstants = computed(() =>
  Math.max(0, data.value.constants.length - shownConstants.value.length),
);
</script>

<template>
  <div class="erz-preview">
    <div class="preview-meta">
      <span>{{
        t("package.erz.meta", {
          major: data.layoutMajor,
          minor: data.layoutMinor,
          record: data.layoutRecord,
        })
      }}</span>
      <span>{{ num(data.totalLength) }} B</span>
      <span :class="data.exact ? 'erz-ok' : 'erz-warn'">{{
        data.exact ? t("package.erz.exactOk") : t("package.erz.exactOff")
      }}</span>
    </div>

    <p v-if="!data.layoutSupported" class="erz-notice" role="note">
      {{ t("package.erz.legacyNotice", { record: data.layoutRecord }) }}
    </p>

    <div class="erz-grid" role="table" aria-label="ERZ">
      <div v-for="s in stats" :key="s.label" class="erz-stat" role="row">
        <span class="erz-stat-label">{{ s.label }}</span>
        <span class="erz-stat-value">{{ s.value }}</span>
        <span class="erz-stat-hint">{{ s.hint }}</span>
      </div>
    </div>

    <section v-if="shownConstants.length" class="erz-section">
      <h3 class="erz-h">
        {{ t("package.erz.constantsTitle") }}
        <span class="erz-hint">{{
          t("package.erz.constantsSample", {
            shown: shownConstants.length,
            total: num(data.constantCount),
          })
        }}</span>
      </h3>
      <div class="erz-table-wrap">
        <table class="erz-table">
          <thead>
            <tr>
              <th>{{ t("package.erz.hashColumn") }}</th>
              <th>{{ t("package.erz.valueColumn") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="c in shownConstants" :key="c.hash">
              <td><code>{{ hex32(c.hash) }}</code></td>
              <td><code>{{ c.value }}</code></td>
            </tr>
          </tbody>
        </table>
        <p v-if="hiddenConstants > 0" class="erz-more">
          {{ t("package.erz.moreInHex", { count: num(hiddenConstants) }) }}
        </p>
      </div>
    </section>

    <section v-if="data.strings.length" class="erz-section">
      <h3 class="erz-h">
        {{ t("package.erz.stringsTitle") }}
        <span class="erz-hint">{{
          t("package.erz.stringsSample", { count: data.strings.length })
        }}</span>
      </h3>
      <ul class="erz-strings">
        <li v-for="(s, i) in data.strings" :key="`${s}-${i}`"><code>{{ s }}</code></li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.erz-preview {
  display: grid;
  gap: 12px;
  min-width: 0;
}
.preview-meta {
  color: var(--muted-foreground);
  display: flex;
  flex-wrap: wrap;
  font-size: 11px;
  gap: 10px;
}
.erz-ok {
  color: var(--success, #3f9d63);
}
.erz-warn {
  color: var(--warning, #b98a2f);
}
.erz-notice {
  border: 1px solid var(--warning, #b98a2f);
  border-radius: 8px;
  color: var(--muted-foreground);
  font-size: 12px;
  line-height: 1.6;
  margin: 0;
  padding: 8px 10px;
}
.erz-grid {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
}
.erz-stat {
  border: 1px solid var(--border);
  border-radius: 8px;
  display: grid;
  gap: 2px;
  padding: 8px 10px;
}
.erz-stat-label {
  font-size: 11px;
}
.erz-stat-value {
  font-size: 16px;
  font-variant-numeric: tabular-nums;
}
.erz-stat-hint {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.5;
}
.erz-h {
  font-size: 12px;
  font-weight: 600;
  margin: 0 0 6px;
}
.erz-hint {
  color: var(--muted-foreground);
  font-weight: 400;
}
.erz-table-wrap {
  max-height: 220px;
  overflow: auto;
}
.erz-table {
  border-collapse: collapse;
  font-size: 12px;
  width: 100%;
}
.erz-table th,
.erz-table td {
  border-bottom: 1px solid var(--border);
  padding: 3px 8px;
  text-align: left;
}
.erz-more {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 6px 0 0;
}
.erz-strings {
  column-gap: 14px;
  columns: 3 200px;
  font-size: 12px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.erz-strings code {
  word-break: break-all;
}
</style>
