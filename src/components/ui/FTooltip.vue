<script setup lang="ts">
import { TooltipArrow, TooltipContent, TooltipPortal, TooltipRoot, TooltipTrigger, useForwardPropsEmits } from 'reka-ui'
import { computed } from 'vue'
import { cn } from '@/lib/utils'

/**
 * 公共 Tooltip（shadcn/reka-ui 包装 + design token）：内容走 content 插槽，
 * 缺省展示 text 属性；side/delayDuration 等透传 Root/Content 属性。样式
 * 全部走 tokens.css 变量（--popover/--foreground/--border），无硬编码色。
 */
interface Props {
  text?: string
  side?: 'top' | 'right' | 'bottom' | 'left'
  delayDuration?: number
  class?: string
}
const props = withDefaults(defineProps<Props>(), {
  text: '',
  side: 'top',
  delayDuration: 300,
})
const emits = defineEmits<{ 'update:open': [open: boolean] }>()
const forwarded = useForwardPropsEmits(
  computed(() => ({ delayDuration: props.delayDuration })),
  emits,
)
</script>

<template>
  <TooltipRoot v-bind="forwarded">
    <TooltipTrigger as-child>
      <slot name="trigger" />
    </TooltipTrigger>
    <TooltipPortal>
      <TooltipContent
        :side="props.side"
        :side-offset="6"
        :class="
          cn(
            'z-50 max-w-72 rounded-md border px-3 py-1.5 text-xs shadow-md',
            props.class,
          )
        "
        :style="{
          background: 'var(--popover, var(--surface-elevated))',
          color: 'var(--popover-foreground, var(--foreground))',
          borderColor: 'var(--border)',
        }"
      >
        <slot>{{ props.text }}</slot>
        <TooltipArrow
          :style="{
            fill: 'var(--popover, var(--surface-elevated))',
            stroke: 'var(--border)',
          }"
        />
      </TooltipContent>
    </TooltipPortal>
  </TooltipRoot>
</template>
