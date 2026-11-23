<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';

const props = defineProps<{ path: unknown }>();
const { t } = useI18n();
const runtimePath = computed(() => typeof props.path === 'string' ? props.path.trim() : '');
</script>

<template>
  <aside v-if="runtimePath" class="runtime-notice" :aria-label="t('panelRuntime.required')">
    <strong>{{ t('panelRuntime.required') }}</strong>
    <code class="runtime-path">{{ runtimePath }}</code>
    <p>{{ t('panelRuntime.instruction') }}</p>
    <p>{{ t('panelRuntime.installation') }}</p>
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
