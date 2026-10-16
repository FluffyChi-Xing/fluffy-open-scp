<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import EvidenceValue from "./EvidenceValue.vue";
const props = defineProps<{ operations: unknown; query: string }>();
const { t } = useI18n();
const object = (v: unknown): Record<string, unknown> =>
  v !== null && typeof v === "object" && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : {};
const rows = computed(() =>
  (Array.isArray(props.operations) ? props.operations : []).map((value) => {
    const op = object(value),
      before = object(op.before),
      after = object(op.after);
    return {
      resource: op.resourceId,
      before: before.amount,
      after: after.amount,
      delta: op.delta,
      capacityBefore: before.capacity,
      capacityAfter: after.capacity,
      status: op.status,
      matches: op.matchesInt32,
    };
  }),
);
const show = (v: unknown) =>
  typeof v === "number" && Number.isFinite(v)
    ? String(v)
    : t("diagnostics.notKnown");
function status(value: unknown) {
  const labels: Record<string, string> = {
    observed: "effectObserved",
    "read-error": "effectReadError",
    "bin-not-found": "effectBinMissing",
    "not-reached": "effectNotReached",
    "not-mutated-range-condition": "effectRange",
    pending: "effectPending",
  };
  return t(`diagnostics.${labels[String(value)] ?? "notKnown"}`);
}
</script>
<template>
  <div class="effects-scroll">
    <table>
      <caption>
        {{
          t("diagnostics.resourceEffects")
        }}
      </caption>
      <thead>
        <tr>
          <th scope="col">{{ t("diagnostics.effectResource") }}</th>
          <th scope="col">{{ t("diagnostics.effectBefore") }}</th>
          <th scope="col">{{ t("diagnostics.effectAfter") }}</th>
          <th scope="col">{{ t("diagnostics.effectDelta") }}</th>
          <th scope="col">{{ t("diagnostics.effectCapacityBefore") }}</th>
          <th scope="col">{{ t("diagnostics.effectCapacityAfter") }}</th>
          <th scope="col">{{ t("diagnostics.effectStatus") }}</th>
          <th scope="col">{{ t("diagnostics.effectCheck") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(row, index) in rows" :key="index">
          <td><EvidenceValue :value="row.resource" :query="query" /></td>
          <td>{{ show(row.before) }}</td>
          <td>{{ show(row.after) }}</td>
          <td>
            <strong>{{ show(row.delta) }}</strong>
          </td>
          <td>{{ show(row.capacityBefore) }}</td>
          <td>{{ show(row.capacityAfter) }}</td>
          <td>{{ status(row.status) }}</td>
          <td>
            {{
              t(
                row.matches === true
                  ? "diagnostics.effectMatch"
                  : row.matches === false
                    ? "diagnostics.effectMismatch"
                    : "diagnostics.notKnown",
              )
            }}
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
<style scoped>
.effects-scroll {
  max-width: 100%;
  overflow-x: auto;
}
table {
  border-collapse: collapse;
  font-variant-numeric: tabular-nums;
}
caption {
  text-align: start;
}
th,
td {
  padding: 0.35rem 0.6rem;
  text-align: start;
  border-bottom: 1px solid var(--border);
  white-space: nowrap;
}
</style>
