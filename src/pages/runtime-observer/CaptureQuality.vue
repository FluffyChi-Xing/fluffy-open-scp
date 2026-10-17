<script setup lang="ts">
import LocalFilePicker from "./LocalFilePicker.vue";
import FAlert from "@/components/ui/FAlert.vue";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { captureQuality } from "./captureQuality";
import type { Raw } from "./diagnostics";
const props = defineProps<{ status?: Raw; records: number; busy: boolean }>();
const emit = defineEmits<{ import: [file: File] }>();
const { t } = useI18n();
const quality = computed(() =>
  props.status ? captureQuality(props.status, props.records) : undefined,
);
const counters = [
  "reported",
  "dropped",
  "decodeErrors",
  "errors",
  "hostErrors",
  "incompleteRequests",
] as const;
const columns = ["hits", "accepted", "filtered", "dropped", "queued"] as const;
</script>
<template>
  <section class="quality" :aria-label="t('diagnostics.captureQuality')">
    <LocalFilePicker
      :label="t('diagnostics.importStatus')"
      accept=".json"
      :disabled="busy"
      @select="emit('import', $event)"
    />
    <FAlert type="warning" class="quality-warning" role="status"
      ><div>{{ t("diagnostics.quality") }}</div>
      <div>{{ t("diagnostics.statusAssociation") }}</div></FAlert
    >
    <template v-if="quality">
      <dl>
        <template v-for="key in counters" :key="key"
          ><dt>{{ t(`diagnostics.${key}`) }}</dt>
          <dd>{{ quality[key] ?? t("diagnostics.notKnown") }}</dd></template
        >
        <dt>{{ t("diagnostics.detached") }}</dt>
        <dd>
          {{
            quality.detached === undefined
              ? t("diagnostics.notKnown")
              : t(quality.detached ? "diagnostics.yes" : "diagnostics.no")
          }}
        </dd>
      </dl>
      <p v-if="quality.countMatches === false" role="alert">
        {{ t("diagnostics.countMismatch") }}
      </p>
      <table v-if="quality.channels.length">
        <thead>
          <tr>
            <th>{{ t("diagnostics.channel") }}</th>
            <th v-for="key in columns" :key="key">
              {{ t(`diagnostics.${key}`) }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="channel in quality.channels" :key="channel.name">
            <th>{{ channel.name }}</th>
            <td v-for="key in columns" :key="key">
              {{ channel[key] ?? t("diagnostics.notKnown") }}
            </td>
          </tr>
        </tbody>
      </table>
      <details>
        <summary>{{ t("diagnostics.rawStatus") }}</summary>
        <pre>{{ JSON.stringify(status, null, 2) }}</pre>
      </details>
    </template>
    <p v-else>{{ t("diagnostics.notKnown") }}</p>
  </section>
</template>
<style scoped>
.quality {
  border: 1px solid var(--border);
  padding: 8px;
  font-size: 12px;
  overflow: auto;
}
.quality-warning {
  margin-top: 6px;
  border: 1px solid color-mix(in srgb, var(--warning) 42%, var(--border));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--warning) 13%, var(--surface));
  color: color-mix(in srgb, var(--warning) 70%, var(--foreground));
}
.quality-warning :deep(.f-alert-content) {
  white-space: normal;
  line-height: 1.5;
}
dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 6px 16px;
}
dd {
  margin: 0;
}
th,
td {
  padding: 6px 12px;
  text-align: left;
  border-bottom: 1px solid var(--border);
}
pre {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-height: 240px;
  overflow: auto;
}
[role="alert"] {
  color: var(--danger);
}
</style>
