<script setup lang="ts">
import type { StepperItemProps } from 'reka-ui'
import { StepperItem, useForwardProps } from 'reka-ui'
import { computed } from 'vue'
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<StepperItemProps & { class?: string }>(), {
  step: 1,
})
const delegatedProps = computed(() => {
  const { class: _, ...delegated } = props
  return delegated
})
const forwarded = useForwardProps(delegatedProps)
const classes = computed(() =>
  cn(
    'group/stepper-item relative flex flex-1 items-center justify-center gap-2 rounded-md p-1 text-center',
    props.class,
  ),
)
</script>

<template>
  <StepperItem v-bind="forwarded" :class="classes">
    <slot />
  </StepperItem>
</template>
