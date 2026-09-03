<script setup lang="ts">
import { computed } from 'vue'
import { cn } from '@/lib/utils'

interface Props { class?: string; modelValue?: string | number }
const props = defineProps<Props>()
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()
const classes = computed(() => cn('flex h-9 w-full rounded-md border border-border bg-background px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50', props.class))
</script>

<template><input :value="props.modelValue" :class="classes" @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)" /></template>
