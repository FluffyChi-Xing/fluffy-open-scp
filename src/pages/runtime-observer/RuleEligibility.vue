<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import EvidenceValue from "./EvidenceValue.vue";
const props = defineProps<{ evidence: unknown; query: string }>();
const { t } = useI18n();
const object = (v: unknown): Record<string, unknown> =>
  v !== null && typeof v === "object" && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : {};
const data = computed(() => object(props.evidence));
const diagnosis = computed(() => object(data.value.diagnosis));
const reason = computed(() => {
  const keys: Record<string, string> = {
    allowed: "conditionAllowed",
    "control-flags": "conditionControl",
    "input-shortfall": "conditionInput",
    "capacity-shortfall": "conditionCapacity",
    "range-below": "conditionRangeBelow",
    "range-above": "conditionRangeAbove",
    "minimum-apply": "conditionMinimum",
    "bin-missing": "conditionBinMissing",
    unresolved: "conditionUnresolved",
    "read-error": "effectReadError",
  };
  return t(
    `diagnostics.${keys[String(diagnosis.value.reason)] ?? "conditionUnresolved"}`,
  );
});
const operation = computed(() => {
  const list = data.value.operations,
    ordinal = diagnosis.value.ordinal;
  return Array.isArray(list) &&
    typeof ordinal === "number" &&
    Number.isInteger(ordinal) &&
    ordinal >= 0
    ? object(list[ordinal])
    : {};
});
const bin = computed(() => object(operation.value.bin));
const show = (v: unknown) =>
  typeof v === "number" && Number.isFinite(v)
    ? String(v)
    : t("diagnostics.notKnown");
</script>
<template>
  <div class="eligibility">
    <strong>{{ reason }}</strong>
    <p v-if="operation.resourceId">
      {{ t("diagnostics.effectResource") }}:
      <EvidenceValue :value="operation.resourceId" :query="query" />
    </p>
    <p v-if="operation.status === 'observed'">
      {{
        t("diagnostics.conditionSnapshot", {
          amount: show(bin.amount),
          capacity: show(bin.capacity),
          input: show(operation.input),
          output: show(operation.output),
        })
      }}
    </p>
    <p v-if="data.countCapture === 'captured'">
      {{
        t("diagnostics.conditionCounts", {
          count:
            data.resourceCount === -1
              ? t("diagnostics.conditionUnbounded")
              : show(data.resourceCount),
          minimum: show(data.minApply),
          final: show(data.finalApplyCount),
        })
      }}
    </p>
    <p>{{ t("diagnostics.conditionHint") }}</p>
  </div>
</template>
<style scoped>
.eligibility p {
  margin: 0.3rem 0;
}
</style>
