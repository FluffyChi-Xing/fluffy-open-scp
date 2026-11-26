<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { useI18n } from 'vue-i18n';
import { open } from '@tauri-apps/plugin-dialog';
import { command } from '@/api/tauri';

const props = defineProps<{ required?: boolean; legacyPath?: unknown }>();
const { t } = useI18n();
const needsRuntime = computed(() => props.required || (typeof props.legacyPath === 'string' && props.legacyPath.trim().length > 0));
const exporting = shallowRef(false);
const exportedPath = shallowRef('');
const exportError = shallowRef('');
const exportErrorText = computed(() => {
  const detail = exportError.value;
  const key = `panelEditor.runtimeErrors.${detail}`;
  const message = t(key);
  return t('panelRuntime.exportFailed', { detail: message === key ? detail : message });
});

async function exportRuntime() {
  exporting.value = true;
  exportError.value = '';
  try {
    const gameData = await open({ directory: true, title: t('panelRuntime.selectGameData') });
    if (typeof gameData !== 'string') return;
    const output = await open({ directory: true, title: t('panelRuntime.selectExportFolder') });
    if (typeof output !== 'string') return;
    exportedPath.value = await command<string>('panel_export_shared_runtime', { gameData, output });
  } catch (error) {
    exportError.value = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <aside v-if="needsRuntime" class="runtime-notice" :aria-label="t('panelRuntime.required')">
    <strong>{{ t('panelRuntime.required') }}</strong>
    <p>{{ t('panelRuntime.instruction') }}</p>
    <p>{{ t('panelRuntime.installation') }}</p>
    <button class="mt-2 underline" type="button" :disabled="exporting" @click="exportRuntime">
      {{ t('panelRuntime.export') }}
    </button>
    <code v-if="exportedPath" class="runtime-path" role="status">{{ exportedPath }}</code>
    <p v-if="exportError" role="alert">{{ exportErrorText }}</p>
  </aside>
</template>

<style scoped>
.runtime-notice {
  margin-block: 12px;
  padding: 12px;
  border: 1px solid var(--border, #2b3c54);
  border-radius: 8px;
  font-family: inherit;
  font-size: 11px;
  font-weight: 400;
  line-height: 1.65;
  color: var(--muted-foreground);
}
.runtime-notice strong {
  display: block;
  font-size: 12px;
  font-weight: 600;
  color: var(--foreground);
}
.runtime-path {
  display: block;
  margin-block: 6px 8px;
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, Consolas, monospace);
  font-size: 10.5px;
  font-weight: 400;
  line-height: 1.6;
  white-space: normal;
  overflow-wrap: anywhere;
  user-select: text;
}
.runtime-notice p { margin-block: 4px 0; font: inherit; }
</style>
