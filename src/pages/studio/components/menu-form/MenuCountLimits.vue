<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import { validCountLimit, type CountLimitDraft } from "@/lib/game-ui/count-limits";

const props = defineProps<{ source: string | null }>();
const model = defineModel<CountLimitDraft | null>({ required: true });
const { t } = useI18n();
const store = useUiWorkbenchStore();
const loading = shallowRef(false);
const failed = shallowRef(false);
const retry = shallowRef(0);
const invalid = computed(() => model.value && [model.value.individual, model.value.shared].some((v) => v !== null && !validCountLimit(v)));
watch([() => props.source, retry], async ([source], _, onCleanup) => {
  let cancelled = false;
  onCleanup(() => { cancelled = true; });
  model.value = null;
  loading.value = false;
  failed.value = false;
  if (!source) return;
  loading.value = true;
  try {
    const info = await store.countLimits.load(source);
    if (!cancelled && (info.individual !== null || info.shared !== null)) {
      model.value = { source, individual: info.individual, shared: info.shared };
    }
  } catch {
    if (!cancelled) failed.value = true;
  } finally {
    if (!cancelled) loading.value = false;
  }
}, { immediate: true });
</script>

<template>
  <section v-if="model || loading || failed" class="count-limits">
    <h3>{{ t('studio.workbench.sectionCountLimits') }}</h3>
    <p v-if="loading" role="status">{{ t('studio.workbench.countLimitsLoading') }}</p>
    <template v-else-if="failed">
      <p role="alert">{{ t('studio.workbench.countLimitsLoadFailed') }}</p>
      <button type="button" @click="retry++">{{ t('studio.workbench.countLimitsRetry') }}</button>
    </template>
    <template v-else-if="model">
      <label v-if="model.individual !== null">
        <span>{{ t('studio.workbench.individualCountLimit') }}</span>
        <input v-model.number="model.individual" type="number" min="1" max="16777216" step="1" :aria-invalid="!validCountLimit(model.individual)" />
      </label>
      <label v-if="model.shared !== null">
        <span>{{ t('studio.workbench.sharedCountLimit') }}</span>
        <input v-model.number="model.shared" type="number" min="1" max="16777216" step="1" :aria-invalid="!validCountLimit(model.shared)" />
      </label>
      <p>{{ t('studio.workbench.individualCountHint') }}</p>
      <p v-if="model.shared !== null">{{ t('studio.workbench.sharedCountHint') }}</p>
      <p v-if="invalid" role="alert">{{ t('studio.workbench.countLimitsInvalid') }}</p>
    </template>
  </section>
</template>

<style scoped>
.count-limits { border-top: 1px solid var(--border); padding-top: 16px; display: grid; gap: 10px; }
h3 { font-size: 12px; font-weight: 600; margin: 0; }
label { display: grid; gap: 6px; font-size: 12px; }
input { width: 100%; padding: 8px 10px; color: var(--foreground); background: var(--background); border: 1px solid var(--border); border-radius: var(--radius-md); }
p { margin: 0; color: var(--muted-foreground); font-size: 11px; line-height: 1.5; }
input[aria-invalid="true"] { border-color: var(--destructive); }
</style>
