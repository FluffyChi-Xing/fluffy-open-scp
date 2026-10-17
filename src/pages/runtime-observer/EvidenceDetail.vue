<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { shallowRef } from "vue";
import FCode from "@/components/ui/FCode.vue";
import type { Evidence } from "./diagnostics";
defineProps<{ row: Evidence; paired?: Evidence }>();
const { t } = useI18n();
const rawOpen = shallowRef(false);
const pairOpen = shallowRef(false);
</script>
<template>
  <div class="evidence-detail">
    <p v-if="row.issue">{{ t(`diagnostics.${row.issue}`) }}</p>
    <p v-if="row.changed?.length">
      {{ t("diagnostics.changes") }}: <mark>{{ row.changed.join(", ") }}</mark>
    </p>
    <details @toggle="rawOpen = ($event.target as HTMLDetailsElement).open">
      <summary>{{ t("diagnostics.detail") }}</summary>
      <FCode
        v-if="rawOpen"
        :code="JSON.stringify(row.raw, null, 2)"
        lang="json"
        :copy-label="t('diagnostics.copy')"
        :copied-label="t('diagnostics.copied')"
        :collapse-label="t('diagnostics.collapse')"
        :expand-label="t('diagnostics.expand')"
      />
    </details>
    <details
      v-if="paired"
      @toggle="pairOpen = ($event.target as HTMLDetailsElement).open"
    >
      <summary>{{ t("diagnostics.pair") }} #{{ paired.raw.sequence }}</summary>
      <FCode
        v-if="pairOpen"
        :code="JSON.stringify(paired.raw, null, 2)"
        lang="json"
        :copy-label="t('diagnostics.copy')"
        :copied-label="t('diagnostics.copied')"
        :collapse-label="t('diagnostics.collapse')"
        :expand-label="t('diagnostics.expand')"
      />
    </details>
  </div>
</template>
<style scoped>
.evidence-detail {
  font-size: 12px;
}
.evidence-detail :deep(.f-code) {
  max-height: 320px;
  overflow: auto;
}
summary {
  cursor: pointer;
}
mark {
  color: var(--foreground);
  background: var(--accent);
}
</style>
