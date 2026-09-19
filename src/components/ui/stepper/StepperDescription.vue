<script setup lang="ts">
import type { StepperDescriptionProps } from 'reka-ui'
import { StepperDescription, useForwardProps } from 'reka-ui'
import { computed } from 'vue'
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<StepperDescriptionProps & { class?: string }>(), {})
const delegatedProps = computed(() => {
  const { class: _, ...delegated } = props
  return delegated
})
const forwarded = useForwardProps(delegatedProps)
const classes = computed(() => cn('text-xs text-muted-foreground', props.class))
</script>

<template>
  <StepperDescription v-bind="forwarded" :class="classes">
    <slot />
  </StepperDescription>
</template>
