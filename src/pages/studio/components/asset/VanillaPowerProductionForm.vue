<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useToast } from "@/composables/useToast";
import type { tauriApi } from "@/api";

type Groups = NonNullable<Awaited<ReturnType<typeof tauriApi.workspace.vanillaVariantPowerFields>>>;
const props = defineProps<{ groups: Groups; disabled: boolean }>();
const emit = defineEmits<{ change: [group: string, field: string, value: number] }>();
const { t } = useI18n();
const toast = useToast();

function minimum(field: string) {
  if (field === "windElectricity") return 4;
  return field === "airPollution" || field === "groundPollution" ? 0 : 1;
}

function change(group: string, name: string, event: Event) {
  const input = event.target as HTMLInputElement;
  if (Number.isFinite(input.valueAsNumber) && input.validity.valid) {
    emit("change", group, name, input.valueAsNumber);
  } else {
    input.value = String(props.groups[group][name].value);
    toast.error(t("flow.variant.powerInvalidValue"));
  }
}
</script>

<template>
  <details class="power-production">
    <summary>{{ t("flow.variant.powerProduction") }}</summary>
    <section v-for="(fields, group) in groups" :key="group" class="power-group">
      <h4>{{ t(`flow.variant.powerOwners.${group}`) }}</h4>
      <div class="power-fields">
        <label v-for="(field, name) in fields" :key="name">
          {{ t(`flow.variant.powerFields.${name}`) }}
          <input
            type="number"
            :min="minimum(name)"
            :step="name === 'windElectricity' ? 4 : 1"
            max="16777216"
            :value="field.value"
            :disabled="disabled"
            @change="change(group, name, $event)" />
          <small>{{ t("flow.variant.originalValue", { value: field.original }) }} · <code>{{ field.hash }}</code></small>
        </label>
      </div>
    </section>
  </details>
</template>

<style scoped>
.power-production { grid-column: 1 / -1; min-width: 0; border: 1px solid var(--border); border-radius: var(--radius-md); }
summary { padding: 10px 12px; cursor: pointer; font-weight: 600; }
summary:hover { background: var(--surface-hover); }
.power-group { padding: 12px; border-top: 1px solid var(--border); }
h4 { margin-bottom: 8px; font-size: 13px; font-weight: 600; }
.power-fields { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 190px), 1fr)); gap: 10px; }
label { display: grid; gap: 4px; min-width: 0; color: var(--muted-foreground); font-size: 12px; }
input { width: 100%; padding: 6px 8px; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface-elevated); color: var(--foreground); }
small { font-size: 11px; }
</style>
