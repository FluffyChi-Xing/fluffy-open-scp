<script setup lang="ts">
import type { StepperIndicatorProps } from 'reka-ui'
import { StepperIndicator, useForwardProps } from 'reka-ui'
import { computed } from 'vue'
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<StepperIndicatorProps & { class?: string }>(), {})
const delegatedProps = computed(() => {
  const { class: _, ...delegated } = props
  return delegated
})
const forwarded = useForwardProps(delegatedProps)
const classes = computed(() =>
  cn(
    'flex size-7 shrink-0 items-center justify-center rounded-full border-2 border-muted bg-muted font-medium text-muted-foreground/60 transition-colors',
    'group-data-[state=active]/stepper-item:border-primary group-data-[state=active]/stepper-item:bg-primary group-data-[state=active]/stepper-item:text-primary-foreground',
    'group-data-[state=completed]/stepper-item:border-primary group-data-[state=completed]/stepper-item:bg-primary group-data-[state=completed]/stepper-item:text-primary-foreground',
    props.class,
  ),
)
</script>

<template>
  <StepperIndicator v-slot="slotProps" v-bind="forwarded" :class="classes">
    {{ slotProps.step }}
  </StepperIndicator>
</template>
