<script setup lang="ts">
import { TooltipArrow, TooltipContent, TooltipPortal, TooltipProvider, TooltipRoot, TooltipTrigger } from 'reka-ui'
import { cn } from '@/lib/utils'

/**
 * 公共 Tooltip（shadcn/reka-ui 包装 + design token）：内容走 content 插槽，
 * 缺省展示 text 属性；side 透传 Content，delayDuration 给 Provider。样式
 * 全部走 tokens.css 变量（--popover/--foreground/--border），无硬编码色。
 * 自包含 Provider——reka-ui 2.x 的 TooltipRoot 必须在 Provider 内（缺失时
 * 注入失败会炸掉整棵挂载子树）。
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
</script>

<template>
  <TooltipProvider :delay-duration="props.delayDuration">
    <TooltipRoot>
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
  </TooltipProvider>
</template>
