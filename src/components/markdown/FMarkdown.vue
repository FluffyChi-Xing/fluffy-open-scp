<script setup lang="ts">
import { computed, h, nextTick, onBeforeUnmount, onMounted, ref, render, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import MarkdownIt from 'markdown-it'
import FCode from '@/components/ui/FCode.vue'

interface Props { source: string }
const props = defineProps<Props>()
const { t } = useI18n()
const container = ref<HTMLElement>()
const mountedHosts: HTMLElement[] = []

const md = new MarkdownIt({ html: false, linkify: true, breaks: false, typographer: false })
md.renderer.rules.fence = (tokens, idx) => {
  const token = tokens[idx]
  const lang = (token.info || '').trim().split(/\s+/)[0] || ''
  return `<div class="f-md-code" data-lang="${escapeAttr(lang)}">${escapeHtml(token.content)}</div>`
}
const html = computed(() => md.render(props.source))

function mountCodeBlocks() {
  mountedHosts.splice(0).forEach((host) => render(null, host))
  const root = container.value
  if (!root) return
  root.querySelectorAll('.f-md-code').forEach((node) => {
    const host = document.createElement('div')
    node.replaceWith(host)
    mountedHosts.push(host)
    render(h(FCode, {
      code: node.textContent ?? '',
      lang: node.getAttribute('data-lang') ?? '',
      copyLabel: t('code.copy'),
      copiedLabel: t('code.copied'),
      collapseLabel: t('code.collapse'),
      expandLabel: t('code.expand')
    }), host)
  })
}

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char] as string)
}
function escapeAttr(value: string) {
  return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char] as string)
}

watch(() => props.source, async () => { await nextTick(); mountCodeBlocks() })
onMounted(async () => { await nextTick(); mountCodeBlocks() })
onBeforeUnmount(() => { mountedHosts.splice(0).forEach((host) => render(null, host)) })
</script>

<template>
  <div ref="container" class="f-markdown" v-html="html"></div>
</template>

<style scoped>
.f-markdown{color:var(--foreground);font-size:14px;line-height:1.7}.f-markdown>:first-child{margin-top:0}.f-markdown>:last-child{margin-bottom:0}.f-markdown h1,.f-markdown h2,.f-markdown h3,.f-markdown h4{line-height:1.3;letter-spacing:-.02em;margin:1.6em 0 .6em}.f-markdown h1{font-size:1.7em}.f-markdown h2{border-bottom:1px solid var(--border);font-size:1.35em;padding-bottom:.3em}.f-markdown h3{font-size:1.15em}.f-markdown h4{font-size:1em}.f-markdown p{margin:0 0 1em}.f-markdown a{color:var(--primary);text-decoration:none}.f-markdown a:hover{text-decoration:underline}.f-markdown ul,.f-markdown ol{margin:0 0 1em;padding-inline-start:1.4em}.f-markdown li{margin:.35em 0}.f-markdown blockquote{border-inline-start:3px solid var(--border-strong);color:var(--muted-foreground);margin:0 0 1em;padding:.15em 0 .15em 1em}.f-markdown code{background:var(--surface-hover);border-radius:var(--radius-sm);color:var(--foreground);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:.88em;padding:.12em .35em}.f-markdown pre{margin:0 0 1em}.f-markdown hr{border:0;border-top:1px solid var(--border);margin:1.6em 0}.f-markdown table{border-collapse:collapse;display:block;margin:0 0 1em;max-width:100%;overflow-x:auto}.f-markdown th,.f-markdown td{border:1px solid var(--border);padding:.45em .7em;text-align:start}.f-markdown th{background:var(--surface-hover);font-weight:700}.f-markdown .f-md-code{margin:0 0 1em}
</style>
