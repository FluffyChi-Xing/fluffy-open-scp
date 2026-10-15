<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { codeToHtml, codeToTokens } from 'shiki'
import { diffLines, type DiffRow } from '@/lib/lineDiff'

interface Props {
  code: string
  /** 提供时渲染 unified diff（baseCode → code），+/− 行与双侧行号。 */
  baseCode?: string
  /** 行号栏（diff 视图恒有行号，此开关只作用于普通视图）。 */
  lineNumbers?: boolean
  lang?: string
  copyLabel?: string
  copiedLabel?: string
  collapseLabel?: string
  expandLabel?: string
}

const props = withDefaults(defineProps<Props>(), {
  baseCode: '',
  lineNumbers: false,
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
const lineCount = computed(() => (props.code ? props.code.split('\n').length : 0))
/** diff 视图行（token 预着色；null = 非纯文本回退/非 diff 模式）。 */
interface DiffRowView extends DiffRow {
  spans: { text: string; color?: string }[]
}
const diffRows = ref<DiffRowView[] | null>(null)
let highlightToken = 0
let copiedTimer: ReturnType<typeof setTimeout> | undefined
// data-theme 是非响应式 DOM 属性，Vue watch 追踪不到——用 MutationObserver
// 监听切换（此前主题切换后高亮停留旧主题，日间模式呈现暗色 token 不可读）。
let themeObserver: MutationObserver | undefined

function themeName() { return document.documentElement.dataset.theme === 'dark' ? 'github-dark' : 'github-light' }

/** codeToTokens 的 lang 形参比 codeToHtml 严（联合字面量），统一收口。 */
type ShikiLang = NonNullable<Parameters<typeof codeToTokens>[1]>['lang']
function langOf() { return (props.lang || 'text') as ShikiLang }

/** token 行取色失败/缺席 → 单 span 纯文本（diff 永不因高亮失败而空白）。 */
function plainSpans(rows: DiffRow[]): DiffRowView[] {
  return rows.map(row => ({ ...row, spans: [{ text: row.text }] }))
}

async function highlight() {
  const token = ++highlightToken
  if (props.baseCode) {
    diffRows.value = null
    if (!props.code && !props.baseCode) return
  try {
    const theme = themeName()
    const lang = langOf()
    const [before, after] = await Promise.all([
      codeToTokens(props.baseCode, { lang, theme }),
      codeToTokens(props.code, { lang, theme })
    ])
      const rows = diffLines(props.baseCode, props.code)
      const views = rows.map((row): DiffRowView => {
        const line =
          row.type === 'del'
            ? before.tokens[row.oldNo! - 1]
            : after.tokens[row.newNo! - 1]
        const spans = line?.map(tok => ({ text: tok.content, color: tok.color }))
        return { ...row, spans: spans?.length ? spans : [{ text: row.text }] }
      })
      if (token === highlightToken) diffRows.value = views
      return
    } catch {
      if (token === highlightToken) diffRows.value = plainSpans(diffLines(props.baseCode, props.code))
      return
    }
  }
  diffRows.value = null
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

watch(() => [props.code, props.baseCode, props.lang], highlight, { immediate: true })
onMounted(() => {
  themeObserver = new MutationObserver(() => highlight())
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
})
onBeforeUnmount(() => {
  themeObserver?.disconnect()
  if (copiedTimer) clearTimeout(copiedTimer)
})
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
      <!-- diff 视图：旧|新双行号 + 标记列 + token 着色行 -->
      <div v-if="diffRows" class="f-code-diff" role="table" aria-label="diff">
        <div v-for="(row, index) in diffRows" :key="index" class="f-code-diff-row" :class="`is-${row.type}`">
          <span class="f-code-diff-no" aria-hidden="true">{{ row.oldNo ?? '' }}</span>
          <span class="f-code-diff-no" aria-hidden="true">{{ row.newNo ?? '' }}</span>
          <span class="f-code-diff-marker" aria-hidden="true">{{ row.type === 'add' ? '+' : row.type === 'del' ? '−' : '' }}</span>
          <span class="f-code-diff-text"><span v-for="(tok, tokIndex) in row.spans" :key="tokIndex" :style="tok.color ? { color: tok.color } : undefined">{{ tok.text }}</span></span>
        </div>
      </div>
      <div v-else-if="highlighted" class="f-code-lines" :class="{ guttered: props.lineNumbers }">
        <div v-if="props.lineNumbers" class="f-code-gutter" aria-hidden="true">
          <span v-for="n in lineCount" :key="n">{{ n }}</span>
        </div>
        <div class="f-code-shiki" v-html="highlighted"></div>
      </div>
      <pre v-else class="f-code-plain"><code>{{ plain }}</code></pre>
    </div>
  </section>
</template>

<style scoped>
.f-code{background:var(--surface-elevated);border:1px solid var(--border);border-radius:var(--radius-lg);box-shadow:var(--shadow-sm);overflow:hidden}.f-code-header{align-items:center;display:flex;gap:10px;padding:8px 12px}.f-code-dots{display:flex;gap:7px}.f-code-dot{border:0;border-radius:50%;cursor:pointer;height:11px;padding:0;transition:filter 120ms ease,scale 120ms ease;width:11px}.f-code-dot:hover{filter:brightness(.92);scale:1.18}.f-code-dot-red{background:#ff5f57}.f-code-dot-yellow{background:#febc2e}.f-code-dot-green{background:#28c840}.f-code-lang{color:var(--muted-foreground);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:11px;margin-inline-start:auto}.f-code-copy{align-items:center;background:transparent;border:0;border-radius:var(--radius-sm);color:var(--muted-foreground);cursor:pointer;display:inline-flex;font-size:11px;font-weight:650;gap:5px;padding:4px 7px;transition:background-color 120ms ease,color 120ms ease}.f-code-copy:hover{background:var(--surface-hover);color:var(--foreground)}.f-code-copy svg{fill:none;height:13px;stroke:currentColor;stroke-linecap:round;stroke-linejoin:round;stroke-width:1.8;width:13px}.f-code-body{border-top:1px solid var(--border)}.f-code-plain{margin:0;overflow-x:auto;padding:14px 16px}.f-code-plain code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;line-height:1.6;white-space:pre}.f-code-shiki :deep(pre){background:transparent!important;margin:0;overflow-x:auto;padding:14px 16px}.f-code-shiki :deep(code){font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;line-height:1.6}
/* 行号栏：与代码同容器纵向滚动，代码列独立横向滚动；行高/字号/内边距
   与 shiki pre 逐值一致保证逐行对齐。无行号（非 guttered）保持块级布局 */
.f-code-lines.guttered{display:grid;grid-template-columns:auto minmax(0,1fr)}
.f-code-lines.guttered .f-code-shiki :deep(pre){overflow-x:auto}
.f-code-gutter{border-inline-end:1px solid var(--border);color:var(--subtle-foreground);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;line-height:1.6;overflow:hidden;padding:14px 0;text-align:end;user-select:none}
.f-code-gutter span{display:block;padding-inline-end:10px}
/* diff 视图：+/− 行底色取 git 语义；横向滚动整行一体 */
.f-code-diff{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;line-height:1.6;overflow-x:auto;padding:8px 0}
.f-code-diff-row{display:flex;min-height:1.6em;min-width:max-content;padding:0 12px 0 0}
.f-code-diff-row.is-add{background:color-mix(in srgb,#28c840 14%,transparent)}
.f-code-diff-row.is-del{background:color-mix(in srgb,#ff5f57 13%,transparent)}
.f-code-diff-no{color:var(--subtle-foreground);overflow:hidden;padding-inline-end:10px;text-align:end;user-select:none;width:44px;flex:none}
.f-code-diff-marker{flex:none;padding-inline-end:8px;user-select:none}
.f-code-diff-row.is-add .f-code-diff-marker{color:#1a9e34}
.f-code-diff-row.is-del .f-code-diff-marker{color:#d43f38}
.f-code-diff-text{white-space:pre}
</style>
