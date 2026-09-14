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
    host.className = 'f-md-code-host'
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
/*
 * 正文元素全部经 v-html 注入，不带 scoped data 属性；
 * 因此除根节点外一律走 :deep()，否则所有规则（含表格边框）都不会命中。
 */
.f-markdown{color:var(--foreground);font-size:14px;line-height:1.7}
.f-markdown :deep(h1),.f-markdown :deep(h2),.f-markdown :deep(h3),.f-markdown :deep(h4){line-height:1.3;letter-spacing:-.02em;margin:1.6em 0 .6em}
.f-markdown :deep(h1){font-size:1.7em}
.f-markdown :deep(h2){border-bottom:1px solid var(--border);font-size:1.35em;padding-bottom:.3em}
.f-markdown :deep(h3){font-size:1.15em}
.f-markdown :deep(h4){font-size:1em}
.f-markdown :deep(p){margin:0 0 1em}
.f-markdown :deep(a){color:var(--primary);text-decoration:none}
.f-markdown :deep(a:hover){text-decoration:underline}
.f-markdown :deep(ul),.f-markdown :deep(ol){margin:0 0 1em;padding-inline-start:1.4em}
.f-markdown :deep(li){margin:.35em 0}
.f-markdown :deep(blockquote){border-inline-start:3px solid var(--border-strong);color:var(--muted-foreground);margin:0 0 1em;padding:.15em 0 .15em 1em}
.f-markdown :deep(code){background:var(--surface-hover);border-radius:var(--radius-sm);color:var(--foreground);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:.88em;padding:.12em .35em}
.f-markdown :deep(pre){margin:1.25em 0 1.5em}
.f-markdown :deep(hr){border:0;border-top:1px solid var(--border);margin:1.6em 0}
/* 表格：圆角外框 + 单元格网格线。separate+spacing:0 才能让圆角生效，
   末列/末行收掉单侧边框避免与外框叠线；display:block 让宽表横向滚动。
   块级元素（表格/代码）上下各留 1.25em/1.5em：与相邻文字的 margin 折叠后，
   无论前面是段落（1em）还是标题（0.6em），至少拉开 1.25em。 */
.f-markdown :deep(table){border:1px solid var(--border);border-collapse:separate;border-radius:var(--radius-md);border-spacing:0;display:block;margin:1.25em 0 1.5em;max-width:100%;overflow-x:auto;width:max-content}
.f-markdown :deep(th),.f-markdown :deep(td){border-bottom:1px solid var(--border);border-right:1px solid var(--border);padding:.45em .7em;text-align:start;vertical-align:top}
.f-markdown :deep(th:last-child),.f-markdown :deep(td:last-child){border-right:0}
.f-markdown :deep(tbody tr:last-child th),.f-markdown :deep(tbody tr:last-child td){border-bottom:0}
.f-markdown :deep(thead th){background:var(--surface-hover);font-weight:700}
/* 代码块：fence 在水合后由 .f-md-code-host 宿主承担边距（.f-md-code 规则
   只覆盖挂载前一帧，两个类同边距避免跳动）；缩进码块走上面的 pre 规则。 */
.f-markdown :deep(.f-md-code){margin:1.25em 0 1.5em}
.f-markdown :deep(.f-md-code-host){margin:1.25em 0 1.5em}
/* 首尾元素贴住容器：放最后以覆盖上面的块级边距 */
.f-markdown :deep(> :first-child){margin-top:0}
.f-markdown :deep(> :last-child){margin-bottom:0}
</style>
