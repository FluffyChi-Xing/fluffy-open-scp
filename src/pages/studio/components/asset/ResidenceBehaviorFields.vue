<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import FSelect from "@/components/ui/FSelect.vue";
import {
  groundResidence,
  RESIDENCE_CAPACITY_FIELDS,
  RESIDENCE_HAPPINESS_FIELDS,
  type ResidenceParams,
} from "./residenceBehavior";

const props = defineProps<{ params: ResidenceParams; disabled: boolean }>();
const emit = defineEmits<{ patch: [Partial<ResidenceParams>] }>();
const { t } = useI18n();
const profileId = useId();
const keys = "studio.assetWorkbench.behaviorSheet";
const profiles = [
  ["ground-residence-w1-v1", "residenceW1"],
  ["ground-residence-w2-v1", "residenceW2"],
  ["ground-residence-w3-v1", "residenceW3"],
  ["ground-residence-w1-d3-v1", "residenceW1D3"],
  ["copan-residence-v1", "residenceCopan"],
  ["megatower-residence-v1", "residenceMegatower"],
] as const;
const options = computed(() => [
  ...(!props.params.residenceProfile ? [{ value: "", label: t(`${keys}.residenceLegacy`) }] : []),
  ...profiles.map(([value, label]) => ({ value, label: t(`${keys}.${label}`) })),
]);
const defaults = computed(() => groundResidence(props.params));
const capacity = computed(() => props.params.population ?? defaults.value?.population ?? 1100);
const slots = computed(() => {
  const original = defaults.value;
  if (!original) return null;
  const workers = Math.round((original.workers * capacity.value) / original.population);
  const happinessCapacity = capacity.value * 11;
  return {
    residentWorkers: capacity.value - (props.params.residentShoppers ?? capacity.value - workers),
    residentChildren: Math.round((original.children * capacity.value) / original.population),
    residentShoppers: capacity.value - (props.params.residentWorkers ?? workers),
    happinessInitial: Math.min(capacity.value, props.params.happinessCapacity ?? happinessCapacity),
    happinessCapacity,
  };
});

function selectProfile(value: string): void {
  const residenceProfile = profiles.find(([profile]) => profile === value)?.[0];
  const patch: Partial<ResidenceParams> = { residenceProfile };
  for (const field of [...RESIDENCE_CAPACITY_FIELDS, ...RESIDENCE_HAPPINESS_FIELDS]) {
    if (props.params[field] != null) patch[field] = null;
  }
  emit("patch", patch);
}

type Field = "population" | (typeof RESIDENCE_CAPACITY_FIELDS)[number] | (typeof RESIDENCE_HAPPINESS_FIELDS)[number];
function setCount(field: Field, event: Event): void {
  const input = event.target as HTMLInputElement;
  const value = input.value === "" && !input.validity.badInput ? null : input.valueAsNumber;
  const patch: Partial<ResidenceParams> = { [field]: value };
  if (field === "residentWorkers") patch.residentShoppers = null;
  if (field === "residentShoppers") patch.residentWorkers = null;
  emit("patch", patch);
}
</script>

<template>
  <div class="logic-field">
    <label :for="profileId">{{ t(`${keys}.residenceProfile`) }}</label>
    <FSelect
      :id="profileId"
      :model-value="params.residenceProfile ?? ''"
      :options="options"
      :disabled="disabled"
      @change="selectProfile"
    />
  </div>
  <label class="logic-field">
    <span>{{ t(`${keys}.population`) }}</span>
    <input
      type="number" min="1" max="100000" step="1" :disabled="disabled"
      :placeholder="String(defaults?.population ?? 1100)" :value="params.population ?? ''"
      @change="setCount('population', $event)" />
  </label>
  <template v-if="slots">
    <label v-for="field in RESIDENCE_CAPACITY_FIELDS" :key="field" class="logic-field">
      <span>{{ t(`${keys}.${field}`) }}</span>
      <input
        type="number" min="0" :max="capacity" step="1" :disabled="disabled"
        :placeholder="String(slots[field])" :value="params[field] ?? ''" @change="setCount(field, $event)" />
    </label>
    <label v-for="field in RESIDENCE_HAPPINESS_FIELDS" :key="field" class="logic-field">
      <span>{{ t(`${keys}.${field}`) }}</span>
      <input
        type="number" :min="field === 'happinessInitial' ? 0 : 1"
        :max="field === 'happinessInitial' ? params.happinessCapacity ?? slots.happinessCapacity : 1100000"
        step="1" :disabled="disabled"
        :placeholder="String(slots[field])" :value="params[field] ?? ''" @change="setCount(field, $event)" />
    </label>
  </template>
</template>

<style scoped>
.logic-field {
  align-items: center;
  display: grid;
  font-size: 10.5px;
  gap: 6px;
  grid-template-columns: 112px minmax(0, 1fr);
}
.logic-field > span,
.logic-field > label {
  color: var(--flow-text-dim, #9b9b9b);
}
.logic-field input {
  background: var(--flow-field, #1a1a1a);
  border: 1px solid var(--flow-field-border, #3a3a3a);
  border-radius: 4px;
  color: var(--flow-text, #e6e6e6);
  font: inherit;
  font-size: 11px;
  height: 26px;
  min-width: 0;
  padding: 0 6px;
}
</style>
