<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import FSelect, { type SelectOption } from "@/components/ui/FSelect.vue";
const props = defineProps<{ targets: { id: string; label: string }[] }>();
const emit = defineEmits<{ move: [target: string] }>();
const target = shallowRef("");
const { t } = useI18n();
const options = computed<SelectOption[]>(() => [
  { value: "", label: t('studio.workbench.copyTargetPlaceholder') },
  ...props.targets.map(item => ({ value: item.id, label: item.label })),
]);
</script>

<template>
  <section class="move-control">
    <label class="move-label">
      <span>{{ t('studio.workbench.moveTo') }}</span>
      <FSelect v-model="target" :options="options" />
    </label>
    <button type="button" :disabled="!target" @click="emit('move', target); target = ''">{{ t('studio.workbench.moveAction') }}</button>
    <p>{{ t('studio.workbench.moveHint') }}</p>
  </section>
</template>

<style scoped>
.move-control { display: grid; grid-template-columns: 1fr auto; gap: 8px; margin-top: 10px; }
.move-label { display: grid; gap: 4px; font-size: 11px; }
.move-control :deep(.f-select), .move-control button { border: 1px solid var(--border); background: var(--surface); color: var(--foreground); border-radius: var(--radius-sm); font: inherit; font-size: 12px; min-height: 32px; padding: 0 9px; min-width: 0; }
.move-control :deep(.f-select) { height: 32px; }
.move-control button { align-self: end; }
.move-control button:disabled { opacity: .45; }
.move-control p { grid-column: 1 / -1; color: var(--muted-foreground); font-size: 11px; line-height: 1.45; }
</style>
