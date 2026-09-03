<script setup lang="ts">
import { computed, shallowRef } from 'vue'

interface EllipsisOptions { rows?: number; expandable?: boolean; expandText?: string; collapseText?: string }
interface Props { ellipsis?: boolean | EllipsisOptions; expanded?: boolean }
const props = defineProps<Props>()
const emit = defineEmits<{ 'update:expanded': [value: boolean]; expand: []; collapse: [] }>()
const localExpanded = shallowRef(props.expanded ?? false)
const options = computed<EllipsisOptions>(() => props.ellipsis === true ? {} : props.ellipsis || {})
const rows = computed(() => options.value.rows ?? 1)
const isExpanded = computed(() => props.expanded ?? localExpanded.value)
const style = computed(() => isExpanded.value || !props.ellipsis ? undefined : { WebkitLineClamp: rows.value })
function toggle() {
  const value = !isExpanded.value
  if (props.expanded === undefined) localExpanded.value = value
  emit('update:expanded', value)
  if (value) emit('expand')
  else emit('collapse')
}
</script>

<template>
  <p class="f-typography-paragraph" :class="{ clamped: props.ellipsis && !isExpanded }" :style="style"><slot /></p>
  <button v-if="options.expandable" class="f-typography-toggle" type="button" @click="toggle">{{ isExpanded ? options.collapseText ?? '收起' : options.expandText ?? '展开' }}</button>
</template>

<style scoped>.f-typography-paragraph{line-height:1.7;margin:0;overflow-wrap:anywhere}.clamped{display:-webkit-box;overflow:hidden;-webkit-box-orient:vertical}.f-typography-toggle{background:none;border:0;color:var(--primary);cursor:pointer;font:inherit;margin-top:4px;padding:0}</style>
