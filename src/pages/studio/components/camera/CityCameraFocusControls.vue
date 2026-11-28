<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FButton from "@/components/ui/FButton.vue";
import FSelect from "@/components/ui/FSelect.vue";
import { focusFields } from "./cityCameraFocus";

const props = defineProps<{
  values: Record<number, string>;
  invalid: number[];
  modified: boolean;
  automaticAvailable: boolean;
  disabled: boolean;
}>();
const emit = defineEmits<{ edit: [hash: number, value: string]; reset: [] }>();
const { t } = useI18n();
const modeOptions = computed(() =>
  [
    { value: "0", label: t("studio.camera.focus.off") },
    { value: "1", label: t("studio.camera.focus.auto") },
    { value: "2", label: t("studio.camera.focus.manual") },
  ].filter(
    (option) =>
      option.value !== "1" ||
      props.automaticAvailable ||
      props.values[focusFields[0].hash] === "1",
  ),
);
</script>

<template>
  <section class="focus-controls">
    <h2>{{ t("studio.camera.focus.title") }}</h2>
    <label for="city-camera-focus-mode" class="focus-field">
      {{ t("studio.camera.focus.mode") }}
      <FSelect
        id="city-camera-focus-mode"
        :model-value="values[focusFields[0].hash]"
        :options="modeOptions"
        :disabled="disabled"
        @change="emit('edit', focusFields[0].hash, $event)"
      />
    </label>
    <p v-if="values[focusFields[0].hash] === '1'" class="hint">
      {{
        t(
          automaticAvailable
            ? "studio.camera.focus.autoHint"
            : "studio.camera.focus.missingTable",
        )
      }}
    </p>
    <div v-if="values[focusFields[0].hash] === '2'" class="manual-fields">
      <label
        v-for="field in focusFields.slice(1)"
        :key="field.id"
        class="focus-field"
      >
        {{ t(`studio.camera.focus.${field.id}`) }}
        <input
          type="number"
          :value="values[field.hash]"
          step="any"
          :disabled="disabled"
          :aria-invalid="invalid.includes(field.hash)"
          @input="
            emit('edit', field.hash, ($event.target as HTMLInputElement).value)
          "
        />
      </label>
    </div>
    <p v-if="invalid.length" class="hint" role="status">
      {{ t("studio.camera.focus.invalid") }}
    </p>
    <FButton
      v-if="modified"
      variant="secondary"
      :disabled="disabled"
      @click="emit('reset')"
      >{{ t("studio.camera.focus.reset") }}</FButton
    >
  </section>
</template>

<style scoped>
.focus-controls {
  display: grid;
  gap: 12px;
  padding: 14px 16px;
  border-top: 1px solid var(--border);
}
.focus-controls h2 {
  margin: 0;
  font-size: 15px;
}
.focus-field {
  display: grid;
  gap: 6px;
  font-size: 13px;
}
.focus-field input {
  min-width: 0;
  min-height: 34px;
  padding: 4px 8px;
  color: var(--foreground);
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
.focus-field input[aria-invalid="true"] {
  border-color: var(--destructive);
}
.manual-fields {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(128px, 1fr));
  gap: 12px;
}
.hint {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 12px;
}
.focus-controls button {
  justify-self: start;
}
</style>
