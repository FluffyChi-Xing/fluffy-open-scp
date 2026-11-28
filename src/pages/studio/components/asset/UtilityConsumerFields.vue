<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FSelect from "@/components/ui/FSelect.vue";
import type { UtilityParams } from "./logicGraph";

const props = withDefaults(defineProps<{
  kind: "power" | "water";
  value?: UtilityParams;
  disabled: boolean;
}>(), { value: () => ({ mode: "inherit", amount: null, rate: null }) });
const emit = defineEmits<{ "update:value": [value: UtilityParams] }>();
const { t } = useI18n();
const modes = computed(() => [
  { value: "inherit", label: t("studio.assetWorkbench.utility.modeInherit") },
  { value: "explicit", label: t("studio.assetWorkbench.utility.modeExplicit") },
]);
function changeMode(mode: string): void {
  if (mode === "inherit" || mode === "explicit") emit("update:value", { ...props.value, mode });
}
function changeNumber(key: "amount" | "rate", event: Event): void {
  const number = (event.target as HTMLInputElement).valueAsNumber;
  emit("update:value", { ...props.value, [key]: Number.isFinite(number) ? number : null });
}
</script>

<template>
  <fieldset class="utility-consumer" :disabled="disabled">
    <legend>{{ t(`studio.assetWorkbench.utility.${kind}`) }}</legend>
    <FSelect :model-value="value.mode" :disabled="disabled" :options="modes" @update:model-value="changeMode" />
    <template v-if="value.mode === 'explicit'">
      <label>
        <span>{{ t("studio.assetWorkbench.utility.consumptionAmount") }}</span>
        <input type="number" min="1" step="any" :value="value.amount ?? ''" @change="changeNumber('amount', $event)" />
      </label>
      <label>
        <span>{{ t("studio.assetWorkbench.utility.consumptionInterval") }}</span>
        <input type="number" min="1" step="1" :value="value.rate ?? ''" :placeholder="t('studio.assetWorkbench.utility.intervalInherit')" @change="changeNumber('rate', $event)" />
      </label>
    </template>
    <p>{{ t("studio.assetWorkbench.utility.consumptionHint") }}</p>
  </fieldset>
</template>

<style scoped>
.utility-consumer { display: grid; gap: 8px; margin: 0; padding: 8px; min-width: 0; border: 1px solid var(--border); border-radius: var(--radius-sm); }
.utility-consumer legend { padding: 0 4px; }
.utility-consumer label { display: grid; gap: 4px; }
.utility-consumer input { min-width: 0; padding: 5px 8px; border: 1px solid var(--border); border-radius: var(--radius-sm); color: inherit; background: var(--surface-2); }
.utility-consumer p { margin: 0; font-size: 12px; color: var(--muted-foreground); }
</style>
