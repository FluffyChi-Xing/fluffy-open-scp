<script setup lang="ts">
import { computed, ref } from "vue";
import FIcon from "../extensions/FIcon.vue";
import FDropdown from "./FDropdown.vue";

export interface SelectOption {
  label: string;
  value: string;
}
interface Props {
  id?: string;
  options: SelectOption[];
  disabled?: boolean;
  invalid?: boolean;
}
const props = defineProps<Props>();
const model = defineModel<string>({ default: "" });
const open = ref(false);
const emit = defineEmits<{ change: [value: string] }>();

/**
 * 项目约定：禁止原生 <select>。本组件对外保持既有 props/events API
 * （options/model/invalid/disabled + change），内部由 FDropdown 渲染。
 */
const selected = computed(() =>
  props.options.find((option) => option.value === model.value),
);

function choose(option: SelectOption) {
  model.value = option.value;
  open.value = false;
  emit("change", option.value);
}
</script>

<template>
  <FDropdown v-model:open="open" :width="220" class="f-select-anchor"
    ><template #trigger
      ><button
        :id="props.id"
        type="button"
        class="f-select"
        :class="{ invalid: props.invalid }"
        :disabled="props.disabled"
        :aria-invalid="props.invalid || undefined"
        :aria-haspopup="'listbox'"
      >
        <span class="f-select-label" :class="{ placeholder: !selected }">{{
          selected?.label ?? model ?? ""
        }}</span
        ><FIcon
          name="ChevronDown"
          :size="13"
          aria-label=""
        /></button></template
    ><button
      v-for="option in props.options"
      :key="option.value"
      type="button"
      @click="choose(option)"
    >
      <FIcon
        :name="option.value === model ? 'Check' : 'Minus'"
        :size="14"
        aria-label=""
      />{{ option.label }}
    </button></FDropdown
  >
</template>

<style scoped>
.f-select {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  gap: 8px;
  justify-content: space-between;
  min-height: 38px;
  outline: 0;
  padding: 0 10px;
  width: 100%;
}
.f-select:hover:not(:disabled) {
  border-color: var(--border-strong);
}
.f-select:focus {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--ring) 25%, transparent);
}
.f-select.invalid {
  border-color: var(--danger);
}
.f-select:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.f-select-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.f-select-label.placeholder {
  color: var(--muted-foreground);
}
</style>
