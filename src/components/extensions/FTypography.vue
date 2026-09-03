<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import FTypographyParagraph from './FTypographyParagraph.vue'

type TypographyType = 'default' | 'secondary' | 'success' | 'warning' | 'danger'
interface EllipsisOptions { rows?: number; expandable?: boolean; expandText?: string; collapseText?: string }
interface Props { header?: 1 | 2 | 3 | 4 | 5 | 6; paragraphy?: boolean; type?: TypographyType; spacing?: 'none' | 'block'; ellipsis?: boolean | EllipsisOptions; expanded?: boolean }
const props = withDefaults(defineProps<Props>(), { type: 'default', spacing: 'block' })
const emit = defineEmits<{ 'update:expanded': [value: boolean]; expand: []; collapse: [] }>()
const tag = computed(() => props.header ? `h${props.header}` : props.paragraphy ? 'p' : 'span')
const classes = computed(() => ['f-typography', `f-typography-${props.type}`, `f-typography-${props.spacing}`, props.header && `f-typography-h${props.header}`])
const paragraphExpanded = ref(props.expanded ?? false)
watch(() => props.expanded, (value) => {
  if (value !== undefined) paragraphExpanded.value = value
})
function updateExpanded(value: boolean) {
  paragraphExpanded.value = value
  emit('update:expanded', value)
}
function expand() { emit('expand') }
function collapse() { emit('collapse') }
</script>

<template>
  <FTypographyParagraph v-if="props.paragraphy && props.ellipsis && !props.header" :ellipsis="props.ellipsis" :expanded="paragraphExpanded" @update:expanded="updateExpanded" @expand="expand" @collapse="collapse"><slot /></FTypographyParagraph>
  <component :is="tag" v-else :class="classes"><slot /></component>
</template>

<style scoped>.f-typography{color:var(--foreground)}.f-typography-secondary{color:var(--muted-foreground)}.f-typography-success{color:var(--success)}.f-typography-warning{color:var(--warning)}.f-typography-danger{color:var(--danger)}.f-typography-block{margin-block:0 1em}.f-typography-none{margin:0}.f-typography-h1,.f-typography-h2,.f-typography-h3,.f-typography-h4,.f-typography-h5,.f-typography-h6{font-weight:700;letter-spacing:-.03em;line-height:1.2}.f-typography-h1{font-size:clamp(2rem,4vw,3rem)}.f-typography-h2{font-size:clamp(1.65rem,3vw,2.25rem)}.f-typography-h3{font-size:1.5rem}.f-typography-h4{font-size:1.25rem}.f-typography-h5{font-size:1.125rem}.f-typography-h6{font-size:1rem}</style>
