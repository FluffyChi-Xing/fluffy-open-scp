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
.runtime-notice { margin-block: 12px; padding: 12px; border: 1px solid var(--border-color); border-radius: 8px; }
.runtime-path { display: block; margin-block: 8px; overflow-wrap: anywhere; user-select: text; }
.runtime-notice p { margin-block: 6px; }
</style>
