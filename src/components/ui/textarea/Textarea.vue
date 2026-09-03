<script setup lang="ts">
import { computed } from 'vue'
import { cn } from '@/lib/utils'

interface Props { class?: string; modelValue?: string }
const props = defineProps<Props>()
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()
const classes = computed(() => cn('flex min-h-20 w-full rounded-md border border-border bg-background px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50', props.class))
</script>

<template><textarea :value="props.modelValue" :class="classes" @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)" /></template>
