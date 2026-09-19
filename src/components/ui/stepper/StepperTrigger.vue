<script setup lang="ts">
import type { StepperTriggerProps } from 'reka-ui'
import { StepperTrigger, useForwardProps } from 'reka-ui'
import { computed } from 'vue'
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<StepperTriggerProps & { class?: string }>(), {})
const delegatedProps = computed(() => {
  const { class: _, ...delegated } = props
  return delegated
})
const forwarded = useForwardProps(delegatedProps)
const classes = computed(() =>
  cn(
    'flex flex-col items-center justify-center gap-1 rounded-md p-1 text-center focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
    props.class,
  ),
)
</script>

<template>
  <StepperTrigger v-bind="forwarded" :class="classes">
    <slot />
  </StepperTrigger>
</template>
