<script setup lang="ts">
import type { StepperTitleProps } from 'reka-ui'
import { StepperTitle, useForwardProps } from 'reka-ui'
import { computed } from 'vue'
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<StepperTitleProps & { class?: string }>(), {})
const delegatedProps = computed(() => {
  const { class: _, ...delegated } = props
  return delegated
})
const forwarded = useForwardProps(delegatedProps)
const classes = computed(() => cn('text-xs font-semibold', props.class))
</script>

<template>
  <StepperTitle v-bind="forwarded" :class="classes">
    <slot />
  </StepperTitle>
</template>
