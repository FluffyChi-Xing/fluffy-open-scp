<script setup lang="ts">
import { computed } from "vue";
import FPopover from "./FPopover.vue";

/**
 * 颜色选择器：色块触发 + 浮动面板（原生色板 + hex 输入）。
 * 值为 "#rrggbb" hex 字符串；调用方负责与自身色彩模型的换算。
 */
const props = withDefaults(
  defineProps<{
    modelValue: string;
    /** 无障碍/提示文案（组件不内置 i18n）。 */
    title?: string;
    /** 触发色块尺寸（px）。 */
    size?: number;
    disabled?: boolean;
  }>(),
  { size: 18, title: "color" },
);
const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();
const open = defineModel<boolean>("open", { default: false });

const normalized = computed(() => {
  const value = props.modelValue.trim();
  return /^#[0-9a-fA-F]{6}$/.test(value) ? value : "#ffffff";
});

function commit(value: string) {
  const trimmed = value.trim();
  if (/^#[0-9a-fA-F]{6}$/.test(trimmed)) emit("update:modelValue", trimmed.toLowerCase());
}
</script>

<template>
  <FPopover v-model:open="open" :width="220">
    <template #trigger>
      <button
        type="button"
        class="color-trigger"
        :class="{ disabled }"
        :aria-label="title"
        :title="title"
        :disabled="disabled"
      >
        <span class="color-swatch" :style="{ background: normalized }" />
      </button>
    </template>
    <div class="color-panel">
      <label class="color-native">
        <input
          type="color"
          :value="normalized"
          @input="commit(($event.target as HTMLInputElement).value)"
        />
        <span class="color-hint">{{ title }}</span>
      </label>
      <label class="color-hex">
        <span>HEX</span>
        <input
          type="text"
          :value="normalized"
          maxlength="7"
          spellcheck="false"
          @change="commit(($event.target as HTMLInputElement).value)"
        />
      </label>
    </div>
  </FPopover>
</template>

<style scoped>
.color-trigger {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: inline-flex;
  padding: 3px;
  width: 100%;
}
.color-trigger.disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.color-swatch {
  border-radius: calc(var(--radius-sm) - 2px);
  display: block;
  height: calc(v-bind(size) * 1px);
  width: 100%;
}
.color-panel {
  display: grid;
  gap: 8px;
  padding: 4px;
}
.color-native {
  cursor: pointer;
  display: grid;
  gap: 6px;
}
.color-native input[type="color"] {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  height: 34px;
  padding: 2px;
  width: 100%;
}
.color-hint {
  color: var(--subtle-foreground);
  font-size: 10.5px;
  text-align: center;
}
.color-hex {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: 28px 1fr;
}
.color-hex span {
  color: var(--subtle-foreground);
  font-size: 10px;
  font-weight: 650;
}
.color-hex input {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  min-height: 26px;
  padding: 0 8px;
  text-transform: lowercase;
  width: 100%;
}
</style>
