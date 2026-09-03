<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { codeToHtml } from 'shiki'

interface Props {
  code: string
  lang?: string
  copyLabel?: string
  copiedLabel?: string
  collapseLabel?: string
  expandLabel?: string
}

const props = withDefaults(defineProps<Props>(), {
  lang: '',
  copyLabel: 'Copy',
  copiedLabel: 'Copied',
  collapseLabel: 'Collapse',
  expandLabel: 'Expand'
})

const collapsed = ref(false)
const copied = ref(false)
const highlighted = ref('')
const plain = computed(() => escapeHtml(props.code))
let highlightToken = 0
let copiedTimer: ReturnType<typeof setTimeout> | undefined

function themeName() { return document.documentElement.dataset.theme === 'dark' ? 'github-dark' : 'github-light' }

async function highlight() {
  const token = ++highlightToken
  if (!props.code) { highlighted.value = ''; return }
  try {
    const html = await codeToHtml(props.code, { lang: props.lang || 'text', theme: themeName() })
    if (token === highlightToken) highlighted.value = html
  } catch {
    if (token === highlightToken) highlighted.value = ''
  }
}

function toggleCollapse() { collapsed.value = !collapsed.value }

async function copy() {
  try {
    await navigator.clipboard.writeText(props.code)
    copied.value = true
    copiedTimer = setTimeout(() => { copied.value = false }, 1500)
  } catch { /* clipboard unavailable */ }
}

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char] as string)
}

watch(() => props.code, highlight, { immediate: true })
watch(() => document.documentElement.dataset.theme, () => { highlight() })
onBeforeUnmount(() => { if (copiedTimer) clearTimeout(copiedTimer) })
</script>

<template>
  <section class="f-code" :class="{ collapsed }">
    <header class="f-code-header">
      <div class="f-code-dots">
        <button type="button" class="f-code-dot f-code-dot-red" :aria-label="collapsed ? props.expandLabel : props.collapseLabel" @click="toggleCollapse" />
        <button type="button" class="f-code-dot f-code-dot-yellow" :aria-label="collapsed ? props.expandLabel : props.collapseLabel" @click="toggleCollapse" />
        <button type="button" class="f-code-dot f-code-dot-green" :aria-label="collapsed ? props.expandLabel : props.collapseLabel" @click="toggleCollapse" />
      </div>
      <span v-if="props.lang" class="f-code-lang">{{ props.lang }}</span>
      <button type="button" class="f-code-copy" @click="copy"><svg v-if="copied" viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12 5 5L20 7" /></svg><svg v-else viewBox="0 0 24 24" aria-hidden="true"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V6a1 1 0 0 1 1-1h9" /></svg><span>{{ copied ? props.copiedLabel : props.copyLabel }}</span></button>
    </header>
    <div v-show="!collapsed" class="f-code-body">
      <div v-if="highlighted" class="f-code-shiki" v-html="highlighted"></div>
      <pre v-else class="f-code-plain"><code>{{ code }}</code></pre>
    </div>
  </section>
</template>

<style scoped>
.f-code{background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow-sm);overflow:hidden}.f-code-header{align-items:center;display:flex;gap:10px;padding:8px 12px}.f-code-dots{display:flex;gap:7px}.f-code-dot{border:0;border-radius:50%;cursor:pointer;height:11px;padding:0;transition:filter 120ms ease,scale 120ms ease;width:11px}.f-code-dot:hover{filter:brightness(.92);scale:1.18}.f-code-dot-red{background:#ff5f57}.f-code-dot-yellow{background:#febc2e}.f-code-dot-green{background:#28c840}.f-code-lang{color:var(--muted-foreground);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:11px;margin-inline-start:auto}.f-code-copy{align-items:center;background:transparent;border:0;border-radius:var(--radius-sm);color:var(--muted-foreground);cursor:pointer;display:inline-flex;font-size:11px;font-weight:650;gap:5px;padding:4px 7px;transition:background-color 120ms ease,color 120ms ease}.f-code-copy:hover{background:var(--surface-hover);color:var(--foreground)}.f-code-copy svg{fill:none;height:13px;stroke:currentColor;stroke-linecap:round;stroke-linejoin:round;stroke-width:1.8;width:13px}.f-code-body{border-top:1px solid var(--border)}.f-code-plain{margin:0;overflow-x:auto;padding:14px 16px}.f-code-plain code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;line-height:1.6;white-space:pre}.f-code-shiki :deep(pre){background:transparent!important;margin:0;overflow-x:auto;padding:14px 16px}.f-code-shiki :deep(code){font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;line-height:1.6}
</style>
