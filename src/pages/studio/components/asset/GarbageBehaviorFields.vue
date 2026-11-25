<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FSelect from "@/components/ui/FSelect.vue";
import type { UtilityParams } from "./logicGraph";

const props = withDefaults(defineProps<{ garbage?: UtilityParams; disabled: boolean }>(), {
  garbage: () => ({ mode: "inherit", amount: null, rate: null }),
});
const emit = defineEmits<{ "update:garbage": [garbage: UtilityParams] }>();
const { t } = useI18n();
const modes = computed(() => [
  { value: "inherit", label: t("studio.assetWorkbench.utility.modeInherit") },
  { value: "explicit", label: t("studio.assetWorkbench.utility.modeExplicit") },
]);

function changeMode(mode: string): void {
  if (mode === "inherit" || mode === "explicit") {
    emit("update:garbage", { ...props.garbage, mode });
  }
}

function changeAmount(event: Event): void {
  const amount = (event.target as HTMLInputElement).valueAsNumber;
  emit("update:garbage", { ...props.garbage, amount: Number.isFinite(amount) ? amount : null });
}
</script>

<template>
  <fieldset class="garbage-fields" :disabled="disabled">
    <legend>{{ t("studio.assetWorkbench.utility.garbage") }}</legend>
    <FSelect
      :model-value="garbage.mode"
      :disabled="disabled"
      :options="modes"
      @update:model-value="changeMode"
    />
    <label v-if="garbage.mode === 'explicit'" class="garbage-amount">
      <span>{{ t("studio.assetWorkbench.utility.garbageAmount") }}</span>
      <input type="number" min="1" step="any" :value="garbage.amount ?? ''" @change="changeAmount" />
    </label>
    <p>{{ t("studio.assetWorkbench.utility.garbageAmountHint") }}</p>
  </fieldset>
</template>

<style scoped>
.garbage-fields {
  display: grid;
  gap: 8px;
  margin: 0;
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  min-width: 0;
}
.garbage-fields legend { padding: 0 4px; }
.garbage-amount { display: grid; gap: 4px; }
.garbage-amount input {
  min-width: 0;
  padding: 5px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: inherit;
  background: var(--surface-2);
}
.garbage-fields p { margin: 0; font-size: 12px; color: var(--muted-foreground); }
</style>
