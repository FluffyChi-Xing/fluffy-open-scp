<script setup lang="ts">
import { computed, ref } from 'vue'
import FIcon from '@/components/extensions/FIcon.vue'

/**
 * 公共 Alert 横幅（design token 化）：全宽信息条，type 决定图标与主色调；
 * closable 显示关闭钮（内置 visible 状态并上抛 close）。内容走默认插槽，
 * 右侧操作区走 action 插槽（如「查看更多」链接）。定位语义（吸顶等）由
 * 使用方通过 class 控制——组件只负责横幅本体。
 */
const props = withDefaults(
  defineProps<{
    type?: 'info' | 'success' | 'warning' | 'danger'
    closable?: boolean
    /** 关闭钮的可读名（aria-label）。 */
    closeLabel?: string
  }>(),
  { type: 'info', closable: false, closeLabel: 'Close' },
)
const emit = defineEmits<{ close: [] }>()

const visible = ref(true)
function dismiss() {
  visible.value = false
  emit('close')
}

const ICONS: Record<string, string> = {
  info: 'Info',
  success: 'CircleCheck',
  warning: 'CircleAlert',
  danger: 'Ban',
}
/* 语义色 token（tokens.css：--info/--success/--warning/--danger），
 * 经 CSS 变量下传给背景/描边/图标的 color-mix 调制 */
const tone = computed(() => `var(--${props.type})`)
</script>

<template>
  <div
    v-if="visible"
    class="f-alert"
    role="alert"
    :data-type="props.type"
    :style="{ '--f-alert-tone': tone }"
  >
    <FIcon :name="ICONS[props.type]" :size="14" class="f-alert-icon" aria-label="" />
    <div class="f-alert-content"><slot /></div>
    <div v-if="$slots.action" class="f-alert-action"><slot name="action" /></div>
    <button
      v-if="props.closable"
      type="button"
      class="f-alert-close"
      :aria-label="props.closeLabel"
      @click="dismiss"
    >
      <FIcon name="X" :size="13" aria-label="" />
    </button>
  </div>
</template>

<style scoped>
.f-alert {
  align-items: center;
  background: color-mix(in srgb, var(--f-alert-tone) 12%, transparent);
  border-bottom: 1px solid color-mix(in srgb, var(--f-alert-tone) 35%, transparent);
  color: var(--foreground);
  display: flex;
  font-size: 12px;
  gap: 8px;
  min-height: 34px;
  padding: 6px 14px;
}
.f-alert-icon {
  color: var(--f-alert-tone);
  flex: none;
}
.f-alert-content {
  margin-inline-end: auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.f-alert-action {
  flex: none;
}
.f-alert-close {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 22px;
  min-width: 22px;
  padding: 0;
}
.f-alert-close:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
</style>
