<script setup lang="ts">
import { Check } from 'lucide-vue-next'
import { CheckboxIndicator, CheckboxRoot } from 'reka-ui'
import { computed } from 'vue'
import { cn } from '@/lib/utils'

type CheckedState = boolean | 'indeterminate'
interface Props { class?: string; modelValue?: CheckedState; disabled?: boolean }
const props = defineProps<Props>()
const emit = defineEmits<{ 'update:modelValue': [value: CheckedState] }>()
const classes = computed(() => cn('peer size-4 shrink-0 rounded-sm border border-border bg-background shadow-sm data-[state=checked]:border-primary data-[state=checked]:bg-primary data-[state=indeterminate]:border-primary data-[state=indeterminate]:bg-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50', props.class))
</script>

<template>
  <CheckboxRoot :model-value="props.modelValue" :disabled="props.disabled" :class="classes" @update:model-value="emit('update:modelValue', $event)">
    <CheckboxIndicator class="flex items-center justify-center text-primary-foreground"><Check class="size-3.5" /></CheckboxIndicator>
  </CheckboxRoot>
</template>
