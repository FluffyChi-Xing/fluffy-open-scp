<script setup lang="ts">
import { computed, ref, nextTick, useTemplateRef, watch } from "vue";
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
const trigger = useTemplateRef<HTMLButtonElement>("trigger");
const optionButtons = useTemplateRef<HTMLButtonElement[]>("optionButtons");
watch(open, async (value) => {
  if (props.disabled) {
    open.value = false;
    return;
  }
  if (value) {
    await nextTick();
    const index = props.options.findIndex(
      (option) => option.value === model.value,
    );
    optionButtons.value?.[Math.max(0, index)]?.focus();
  }
});
watch(
  () => props.disabled,
  (value) => {
    if (value) open.value = false;
  },
);
function navigate(event: KeyboardEvent, index: number) {
  const buttons = optionButtons.value ?? [];
  if (["ArrowDown", "ArrowUp", "Home", "End", "Escape"].includes(event.key)) {
    event.preventDefault();
    event.stopPropagation();
  }
  if (event.key === "ArrowDown") buttons[(index + 1) % buttons.length]?.focus();
  if (event.key === "ArrowUp")
    buttons[(index + buttons.length - 1) % buttons.length]?.focus();
  if (event.key === "Home") buttons[0]?.focus();
  if (event.key === "End") buttons.at(-1)?.focus();
  if (event.key === "Escape") {
    open.value = false;
    trigger.value?.focus();
  }
  if (event.key === "Tab") open.value = false;
}
const emit = defineEmits<{ change: [value: string] }>();

/**
 * 项目约定：禁止原生 <select>。本组件对外保持既有 props/events API
 * （options/model/invalid/disabled + change），内部由 FDropdown 渲染。
 */
const selected = computed(() =>
  props.options.find((option) => option.value === model.value),
);

function choose(option: SelectOption) {
  if (props.disabled) return;
  model.value = option.value;
  open.value = false;
  emit("change", option.value);
  trigger.value?.focus();
}
</script>

<template>
  <div class="f-select-anchor">
    <FDropdown v-model:open="open" :width="280"
      ><template #trigger
        ><button
          :id="props.id"
          ref="trigger"
          type="button"
          class="f-select"
          :class="{ invalid: props.invalid }"
          :disabled="props.disabled"
          :aria-invalid="props.invalid || undefined"
          aria-haspopup="menu"
          :aria-expanded="open"
          @keydown.down.prevent="open = !props.disabled"
          @keydown.up.prevent="open = !props.disabled"
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
        v-for="(option, index) in props.options"
        ref="optionButtons"
        :key="option.value"
        type="button"
        role="menuitemradio"
        :aria-checked="option.value === model"
        @keydown="navigate($event, index)"
        @click="choose(option)"
      >
        <FIcon
          :name="option.value === model ? 'Check' : 'Minus'"
          :size="14"
          aria-label=""
        />{{ option.label }}
      </button></FDropdown
    >
  </div>
</template>

<style scoped>
.f-select-anchor {
  width: 100%;
  min-width: 0;
}
.f-select-anchor :deep(.f-dropdown-anchor) {
  display: flex;
  width: 100%;
}
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
