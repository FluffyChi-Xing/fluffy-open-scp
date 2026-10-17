<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "reka-ui";
import FAlert from "@/components/ui/FAlert.vue";
import FButton from "@/components/ui/FButton.vue";
import FSelect from "@/components/ui/FSelect.vue";
import LocalFilePicker from "./LocalFilePicker.vue";
import EvidenceDetail from "./EvidenceDetail.vue";
import EvidenceValue from "./EvidenceValue.vue";
import CaptureQuality from "./CaptureQuality.vue";
import ResourceEffects from "./ResourceEffects.vue";
import RuleEligibility from "./RuleEligibility.vue";
import { semanticCategory, selectEvidence } from "./diagnostics";
import { useDiagnosticSession } from "./useDiagnosticSession";
const { t } = useI18n();
const {
  session,
  busy,
  error,
  errorOffset,
  errorFile,
  errorHeader,
  importFile,
  importStatus,
  mark,
  download,
} = useDiagnosticSession();
const source = shallowRef("all");
const query = shallowRef("");
const category = shallowRef("all");
const marker = shallowRef("");
const page = shallowRef(0);
const sources = [
  "all",
  "resource",
  "ui",
  "simulation",
  "engine",
  "marker",
  "unknown",
];
const categories = [
  "all",
  "resource-consumption",
  "resource-production",
  "resource-flow",
  "rule-execution",
  "condition-check",
  "resource-source",
  "ui-event",
  "ui-call",
  "engine",
  "marker",
  "unknown",
];
const categoryOptions = computed(() =>
  categories.map((value) => ({
    value,
    label: t(`diagnostics.category.${value}`),
  })),
);
const filtered = computed(() =>
  selectEvidence(session.value?.rows ?? [], source.value, query.value).filter(
    (row) => category.value === "all" || semanticCategory(row) === category.value,
  ),
);
const pages = computed(() =>
  Math.max(1, Math.ceil(filtered.value.length / 100)),
);
const visible = computed(() =>
  filtered.value.slice(page.value * 100, (page.value + 1) * 100),
);
const pairs = computed(
  () =>
    session.value?.rows.filter(
      (r) => r.raw.channel === "result" && r.pair !== undefined,
    ).length ?? 0,
);
watch([source, category, query, session], () => {
  page.value = 0;
});
function addMarker() {
  mark(marker.value);
  marker.value = "";
  source.value = "marker";
}
</script>
<template>
  <div class="diagnostics">
    <section class="diagnostic-controls" :aria-label="t('diagnostics.title')">
      <div class="import-controls">
        <h2>{{ t("diagnostics.title") }}</h2>
        <p>{{ t("diagnostics.hint") }}</p>
        <p>{{ t("diagnostics.deltaHint") }}</p>
        <div class="toolbar">
          <LocalFilePicker
            :label="t('diagnostics.import')"
            accept=".jsonl,.ndjson,.mfs"
            :disabled="busy"
            @select="importFile"
          />
          <span v-if="busy" role="status">{{ t("diagnostics.loading") }}</span>
          <FButton
            :disabled="!session || busy"
            variant="secondary"
            @click="download"
            >{{ t("diagnostics.export") }}</FButton
          >
          <FButton
            :disabled="!session || busy"
            variant="secondary"
            @click="session = undefined"
            >{{ t("diagnostics.clear") }}</FButton
          >
        </div>
        <p v-if="error" role="alert">
          {{ t("diagnostics.fileError", { file: errorFile }) }}
          {{
            t(`diagnostics.${error}`, {
              offset: errorOffset,
              header: errorHeader,
            })
          }}
        </p>
        <p v-if="!session && !error">{{ t("diagnostics.empty") }}</p>
      </div>
    </section>
    <section
      v-if="session"
      class="diagnostic-results"
      :aria-label="t('diagnostics.results')"
    >
      <h2>{{ t("diagnostics.results") }}</h2>
      <FAlert
        type="warning"
        class="warning-note"
        v-if="session.rows.some((r) => r.raw.idRegressed)"
      >
        {{ t("diagnostics.idRegression") }}
      </FAlert>
      <p>
        {{ session.name }} ·
        {{
          t("diagnostics.stats", {
            count: session.rows.length,
            invalid: session.invalid.length,
            pairs,
          })
        }}
      </p>
      <details v-if="session.invalid.length">
        <summary>{{ t("diagnostics.invalid") }}</summary>
        {{ session.invalid.join(", ") }}
      </details>
      <details class="quality-disclosure">
        <summary>{{ t("diagnostics.captureQuality") }}</summary>
        <CaptureQuality
          :status="session.captureStatus"
          :records="session.rows.filter((r) => r.source !== 'marker').length"
          :busy="busy"
          @import="importStatus"
        />
      </details>
      <TabsRoot v-model="source" class="result-tabs">
        <TabsList class="source-tabs" :aria-label="t('diagnostics.title')">
          <TabsTrigger
            v-for="item in sources"
            :key="item"
            :value="item"
            class="source-tab"
            >{{ t(`diagnostics.${item}`) }}</TabsTrigger
          >
        </TabsList>
        <TabsContent :value="source" class="source-panel">
          <input
            v-model="query"
            type="search"
            :aria-label="t('diagnostics.search')"
            :placeholder="t('diagnostics.search')"
          />
          <label class="category-filter">
            <span>{{ t("diagnostics.categoryFilter") }}</span>
            <FSelect
              v-model="category"
              :options="categoryOptions"
              :aria-label="t('diagnostics.categoryFilter')"
            />
          </label>
          <p v-if="source === 'simulation' || source === 'all'">
            {{ t("diagnostics.boundary") }}
          </p>
          <p v-if="source === 'resource'">
            {{ t("diagnostics.resourceHint") }}
          </p>
          <p v-if="source === 'engine'">{{ t("diagnostics.engineHint") }}</p>
          <div class="toolbar">
            <input
              v-model="marker"
              :aria-label="t('diagnostics.markerText')"
              :placeholder="t('diagnostics.markerText')"
              maxlength="2048"
              @keyup.enter="addMarker"
            />
            <FButton
              :disabled="!marker.trim() || busy"
              variant="secondary"
              @click="addMarker"
              >{{ t("diagnostics.addMarker") }}</FButton
            >
          </div>
          <p>{{ t("diagnostics.markerHint") }}</p>

          <ol
            class="evidence-list"
            tabindex="0"
            :aria-label="t('diagnostics.results')"
          >
            <li
              v-for="row in visible"
              :key="row.id"
              class="evidence-row"
              :class="`category-${semanticCategory(row)}`"
            >
              <strong
                >{{ t(`diagnostics.${row.source}`) }} · {{ row.stage }} · #{{
                  row.raw.sequence ?? row.id
                }}
                <span class="category-badge">
                  {{ t(`diagnostics.category.${semanticCategory(row)}`) }}
                </span></strong
              >
              <dl>
                <div
                  v-for="(value, field) in row.fields"
                  :key="field"
                  class="field"
                  :class="{ wide: value !== null && typeof value === 'object' }"
                >
                  <dt>
                    {{
                      [
                        "ruleDefinition",
                        "resourceEffects",
                        "nativeSuccess",
                        "eligibility",
                        "unitSlot",
                        "unitPointer",
                        "unitStamp",
                        "resourceOwner",
                      ].includes(field)
                        ? t(`diagnostics.${field}`)
                        : field
                    }}
                  </dt>
                  <dd>
                    <ResourceEffects
                      v-if="field === 'resourceEffects'"
                      :operations="value"
                      :query="query"
                    />
                    <RuleEligibility
                      v-else-if="field === 'eligibility'"
                      :evidence="value"
                      :query="query"
                    />
                    <span
                      v-else-if="
                        field === 'resourceOwner' && value === 'unit-local'
                      "
                      >{{ t("diagnostics.unitLocalOwner") }}</span
                    >
                    <EvidenceValue v-else :value="value" :query="query" />
                  </dd>
                </div>
              </dl>
              <p v-if="row.fields.ruleDefinition">
                {{ t("diagnostics.ruleDefinitionHint") }}
              </p>
              <p v-if="row.fields.resourceEffects">
                {{ t("diagnostics.resourceEffectsHint") }}
              </p>
              <p v-if="row.fields.resourceOwner === 'unit-local'">
                {{ t("diagnostics.unitEffectsHint") }}
              </p>
              <EvidenceDetail
                :row="row"
                :paired="
                  row.pair === undefined ? undefined : session.rows[row.pair]
                "
              />
            </li>
          </ol>
          <footer class="pagination">
            <FButton
              :disabled="page === 0"
              variant="secondary"
              @click="page--"
              >{{ t("diagnostics.previous") }}</FButton
            >
            <span>{{
              t("diagnostics.page", {
                page: page + 1,
                total: pages,
                count: filtered.length,
              })
            }}</span>
            <FButton
              :disabled="page + 1 >= pages"
              variant="secondary"
              @click="page++"
              >{{ t("diagnostics.next") }}</FButton
            >
          </footer>
        </TabsContent>
      </TabsRoot>
    </section>
  </div>
</template>
<style scoped>
.diagnostics {
  display: grid;
  gap: 12px;
  min-width: 0;
}
.diagnostic-controls,
.diagnostic-results {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  min-width: 0;
}
.import-controls,
.diagnostic-results {
  padding: 12px;
}
.diagnostic-controls {
  overflow: hidden;
}
h2 {
  font-size: 15px;
  margin: 0;
}
p,
label {
  font-size: 12px;
  color: var(--muted-foreground);
  line-height: 1.5;
  margin: 4px 0;
}
.toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  margin: 8px 0;
}
input {
  color: var(--foreground);
  background: var(--background);
  border: 1px solid var(--border);
  padding: 6px 8px;
  font-size: 12px;
  box-sizing: border-box;
  border-radius: var(--radius-sm);
  max-width: 100%;
}
input[type="search"] {
  width: 100%;
  box-sizing: border-box;
}
.category-filter {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 8px 0 0;
}
.category-filter :deep(.f-select-anchor) {
  width: 190px;
}
.category-filter :deep(.f-select) {
  min-width: 190px;
  font-size: 12px;
}
.diagnostics :deep(.f-button) {
  min-height: 28px;
  padding: 0 9px;
  font-size: 12px;
  font-weight: 500;
}
.diagnostic-results {
  max-height: min(82vh, 900px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.diagnostic-results > :not(.result-tabs) {
  flex-shrink: 0;
}
.result-tabs,
.source-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.source-panel > :not(.evidence-list),
.source-tabs {
  flex-shrink: 0;
}
.quality-disclosure[open] {
  max-height: 180px;
  overflow: auto;
}
.source-tabs {
  display: flex;
  gap: 2px;
  overflow-x: auto;
  border-bottom: 1px solid var(--border);
  margin: 8px 0;
}
.source-tab {
  flex: none;
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--muted-foreground);
  padding: 7px 12px;
  font-size: 12px;
  cursor: pointer;
}
.source-tab[data-state="active"] {
  color: var(--primary);
  border-bottom-color: var(--primary);
}
.source-tab:focus-visible,
.evidence-list:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: -2px;
}
.quality-disclosure {
  margin: 6px 0;
  font-size: 12px;
}
summary {
  cursor: pointer;
  color: var(--muted-foreground);
}
.warning-note {
  margin: 6px 0;
  border: 1px solid color-mix(in srgb, var(--warning) 42%, var(--border));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--warning) 13%, var(--surface));
  color: color-mix(in srgb, var(--warning) 70%, var(--foreground));
}
.warning-note :deep(.f-alert-content) {
  white-space: normal;
  line-height: 1.5;
}
.pagination {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--border);
  font-size: 12px;
  font-weight: 400;
  line-height: 1.4;
  color: var(--muted-foreground);
  font-variant-numeric: tabular-nums;
}
.evidence-list {
  list-style: none;
  padding: 0 8px 0 0;
  margin: 8px 0 0;
  max-height: min(48vh, 520px);
  min-height: 100px;
  overflow: auto;
  overscroll-behavior: contain;
}
@media (max-height: 600px) {
  .diagnostic-results {
    overflow: auto;
  }
}
li {
  border-top: 1px solid var(--border);
  padding: 8px 0;
  font-size: 12px;
}
.evidence-row > strong {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.category-badge {
  display: inline-flex;
  align-items: center;
  min-height: 18px;
  padding: 0 7px;
  border: 1px solid currentColor;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 600;
  line-height: 1;
}
.category-resource-consumption,
.category-resource-production,
.category-resource-flow {
  color: #b98525;
}
.category-rule-execution {
  color: #4b8ecf;
}
.category-condition-check {
  color: #b36bce;
}
.category-resource-source {
  color: #2d9a72;
}
.category-ui-event,
.category-ui-call {
  color: #6f78c9;
}
.category-engine {
  color: #c45b5b;
}
.category-marker,
.category-unknown {
  color: var(--muted-foreground);
}
dl {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
  gap: 4px 16px;
  margin: 6px 0;
}
.field {
  display: grid;
  grid-template-columns: minmax(90px, auto) minmax(0, 1fr);
  gap: 8px;
  align-items: baseline;
}
.field.wide {
  grid-column: 1 / -1;
}
dt {
  color: var(--muted-foreground);
}
dd {
  margin: 0;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
  font-family: var(--font-mono);
}
</style>
