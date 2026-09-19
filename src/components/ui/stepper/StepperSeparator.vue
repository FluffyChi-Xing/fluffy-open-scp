<script setup lang="ts">
import type { StepperSeparatorProps } from 'reka-ui'
import { StepperSeparator, useForwardProps } from 'reka-ui'
import { computed } from 'vue'
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<StepperSeparatorProps & { class?: string }>(), {})
const delegatedProps = computed(() => {
  const { class: _, ...delegated } = props
  return delegated
})
const forwarded = useForwardProps(delegatedProps)
const classes = computed(() =>
  cn(
    'm-0.5 h-0.5 w-full shrink-0 rounded-full bg-muted transition-colors',
    'group-data-[state=completed]/stepper-item:bg-primary',
    props.class,
  ),
)
</script>

<template>
  <StepperSeparator v-bind="forwarded" :class="classes">
    <slot />
  </StepperSeparator>
</template>
