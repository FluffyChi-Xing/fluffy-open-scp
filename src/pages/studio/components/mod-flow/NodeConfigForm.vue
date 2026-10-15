<script setup lang="ts">
import FDropdownSelect from "@/components/ui/FSelect.vue";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import type { FlowSchema } from "./contracts";
import MapSourceTool from "./MapSourceTool.vue";
import { useGamePackagesStore } from "@/stores/gamePackages";
const props = defineProps<{ schema: FlowSchema | null; busy: boolean }>();
const emit = defineEmits<{
  save: [schema: FlowSchema];
  dirty: [value: boolean];
  resize: [];
}>();

const { t } = useI18n();
const gamePackages = useGamePackagesStore();
const config = ref<Record<string, string | number>>({});
watch(
  () => JSON.stringify(props.schema?.config),
  () => {
    config.value = { ...props.schema?.config };
  },
  { immediate: true },
);
const dirty = computed(
  () =>
    JSON.stringify(config.value) !== JSON.stringify(props.schema?.config ?? {}),
);
watch(dirty, (value) => emit("dirty", value), { immediate: true });
const fields = computed(() => Object.keys(config.value));
const sourcePackage = computed({
  get: () => String(config.value.package ?? ""),
  set: (value) => {
    config.value.package = value;
  },
});
const sourceGroup = computed({
  get: () => String(config.value.group ?? ""),
  set: (value) => {
    config.value.group = value;
  },
});
const preserveOnly = computed(() =>
  ["city-slots", "resources", "roads", "map-metadata"].includes(
    props.schema?.kind ?? "",
  ),
);
const openedPackageOptions = computed(() =>
  [...gamePackages.opened]
    .sort((a, b) => Number(b.package.packageId === gamePackages.activePackageId) - Number(a.package.packageId === gamePackages.activePackageId))
    .map((entry) => ({
      value: entry.package.path,
      label: entry.package.path.split(/[\\/]/).pop() || entry.package.path,
    })),
);
const sourcePackageChoice = computed({
  get: () => {
    const value = String(config.value.source_package ?? "");
    return value && openedPackageOptions.value.some((option) => option.value === value)
      ? value
      : value
        ? "__manual__"
        : "__none__";
  },
  set: (value: string) => {
    if (value === "__none__") config.value.source_package = "";
    else if (value === "__manual__") {
      if (!String(config.value.source_package ?? "")) config.value.source_package = "";
    } else config.value.source_package = value;
  },
});
const sourcePackageIsManual = computed(() => sourcePackageChoice.value === "__manual__");
function save() {
  if (props.schema)
    emit("save", { ...props.schema, config: { ...config.value } });
}
</script>
<template>
  <form
    v-if="schema"
    class="node-config nodrag nowheel nopan"
    @submit.prevent="save"
  >
    <p v-if="preserveOnly">{{ t("flow.preserveHint") }}</p>
    <p v-if="schema.kind === 'coordinate-alignment'">
      {{ t("flow.alignmentHint") }}
    </p>
    <p v-if="schema.kind === 'map-source'">{{ t("flow.sourceHint") }}</p>
    <p v-if="schema.kind === 'noise-height'">{{ t("flow.generatedHint") }}</p>
    <MapSourceTool
      v-if="schema.kind === 'map-source'"
      v-model:package-path="sourcePackage"
      v-model:group="sourceGroup"
      :disabled="busy"
      @resize="emit('resize')"
    />
    <label v-for="field in fields" :key="field">
      <span>{{ t(`flow.${field}`) }}</span>
      <FDropdownSelect
        v-if="field === 'action'"
        :model-value="String(config[field])"
        :disabled="busy || schema.kind === 'coordinate-alignment'"
        :options="
          ['preserve', 'configure'].map((value) => ({
            value,
            label: t(`flow.${value}`),
          }))
        "
        @update:model-value="config[field] = $event"
      />
      <FDropdownSelect
        v-else-if="field === 'input_mode'"
        :model-value="String(config[field])"
        :disabled="busy"
        :options="[
          { value: 'noise', label: t('flow.noiseInput') },
          { value: 'grayscale', label: t('flow.grayscaleInput') },
        ]"
        @update:model-value="config[field] = $event"
      />
      <FDropdownSelect
        v-else-if="field === 'algorithm'"
        :model-value="String(config[field])"
        :disabled="busy"
        :options="[{ value: 'fbm-v1', label: t('flow.fbmV1') }]"
        @update:model-value="config[field] = $event"
      />
      <input
        v-if="field === 'source_package' && sourcePackageIsManual"
        v-model="config[field]"
        :disabled="busy"
        :placeholder="t('flow.sourcePackageManualHint')"
        spellcheck="false"
      />
      <FDropdownSelect
        v-else-if="field === 'source_package'"
        v-model="sourcePackageChoice"
        :disabled="busy"
        :options="[
          { value: '__none__', label: t('flow.sourcePackageNone') },
          ...openedPackageOptions,
          { value: '__manual__', label: t('flow.sourcePackageManual') },
        ]"
      />
      <input
        v-else-if="typeof config[field] === 'number'"
        v-model.number="config[field]"
        type="number"
        step="any"
        :disabled="busy"
        required
      />
      <input
        v-else
        v-model="config[field]"
        :disabled="busy"
        :required="field !== 'source_package'"
        spellcheck="false"
      />
    </label>
    <button type="submit" :disabled="busy || !dirty">
      {{ t("flow.save") }}
    </button>
  </form>
</template>
<style scoped>
.node-config {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
}
.node-config p {
  margin: 0;
  font-size: 11px;
  line-height: 1.5;
  color: var(--muted-foreground);
}
.node-config label {
  display: grid;
  grid-template-columns: 104px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  font-size: 11px;
}
.node-config input {
  min-width: 0;
  width: 100%;
  box-sizing: border-box;
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface);
  color: var(--foreground);
  font-size: 12px;
}
.node-config :deep(.f-select) {
  min-height: 28px;
  height: 28px;
  font-size: 12px;
}
.node-config > button {
  align-self: flex-end;
  min-height: 26px;
  padding: 0 8px;
  font-size: 11px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-hover);
  color: var(--foreground);
  cursor: pointer;
}
.node-config > button:disabled {
  opacity: 0.45;
  cursor: default;
}
.node-config input:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
</style>
