<script setup lang="ts">
import { computed } from 'vue'
import { CircleHelp } from 'lucide-vue-next'
import { resolveIcon } from '@/lib/icons'

interface Props {
  name: string
  color?: string
  size?: number | string
  strokeWidth?: number
  ariaLabel?: string
}

const props = withDefaults(defineProps<Props>(), { size: 18, strokeWidth: 1.8 })
const icon = computed(() => resolveIcon(props.name) ?? CircleHelp)
const style = computed(() => ({ color: props.color, height: typeof props.size === 'number' ? `${props.size}px` : props.size, width: typeof props.size === 'number' ? `${props.size}px` : props.size }))
</script>

<template>
  <component :is="icon" :style="style" :stroke-width="props.strokeWidth" :aria-hidden="props.ariaLabel ? undefined : 'true'" :aria-label="props.ariaLabel" :role="props.ariaLabel ? 'img' : undefined" />
</template>
