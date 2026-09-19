<script setup lang="ts">
import type { StepperRootEmits, StepperRootProps } from 'reka-ui'
import { StepperRoot, useForwardPropsEmits } from 'reka-ui'
import { computed } from 'vue'
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<StepperRootProps & { class?: string }>(), {
  orientation: 'horizontal',
})
const emits = defineEmits<StepperRootEmits>()

const delegatedProps = computed(() => {
  const { class: _, ...delegated } = props
  return delegated
})
const forwarded = useForwardPropsEmits(delegatedProps, emits)
const classes = computed(() => cn('flex gap-2', props.class))
</script>

<template>
  <StepperRoot v-bind="forwarded" :class="classes">
    <slot />
  </StepperRoot>
</template>
