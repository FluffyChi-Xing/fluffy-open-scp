<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { codeToHtml } from "shiki";
import type { TextPreview } from "@/api/tauri";

/**
 * 长文本不截断预览：行级虚拟滚动（只渲染可视窗口）+ shiki 仅高亮可视片段。
 * 之前整段内容喂给 shiki，资源一大（shader 容器 1MB+）就会卡死渲染。
 * 外观对齐 ui/FCode.vue 的卡片 chrome（三点 + 语言 + 复制）。
 */
const props = defineProps<{ preview: TextPreview }>();

const LINE_HEIGHT = 20;
const OVERSCAN = 24;

const scroller = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewportHeight = ref(420);
const highlighted = ref("");
const copied = ref(false);
let highlightToken = 0;
let scrollFrame = 0;
let copiedTimer: ReturnType<typeof setTimeout> | undefined;

const lines = computed(() =>
  props.preview.content.length ? props.preview.content.split("\n") : [],
);
const totalHeight = computed(() =>
  Math.max(lines.value.length * LINE_HEIGHT, LINE_HEIGHT),
);
const firstVisible = computed(() =>
  Math.max(0, Math.floor(scrollTop.value / LINE_HEIGHT) - OVERSCAN),
);
const endVisible = computed(() =>
  Math.min(
    lines.value.length,
    Math.ceil((scrollTop.value + viewportHeight.value) / LINE_HEIGHT) + OVERSCAN,
  ),
);
const visibleCode = computed(() =>
  lines.value.slice(firstVisible.value, endVisible.value).join("\n"),
);
const offsetStyle = computed(() => ({
  transform: `translateY(${firstVisible.value * LINE_HEIGHT}px)`,
}));

function themeName() {
  return document.documentElement.dataset.theme === "dark"
    ? "github-dark"
    : "github-light";
}

async function highlight() {
  const token = ++highlightToken;
  const language = props.preview.language;
  if (!visibleCode.value || !language || language === "text") {
    highlighted.value = "";
    return;
  }
  try {
    const html = await codeToHtml(visibleCode.value, {
      lang: language,
      theme: themeName(),
    });
    if (token === highlightToken) highlighted.value = html;
  } catch {
    if (token === highlightToken) highlighted.value = "";
  }
}

function onScroll() {
  if (scrollFrame) return;
  scrollFrame = requestAnimationFrame(() => {
    scrollFrame = 0;
    const element = scroller.value;
    if (element) scrollTop.value = element.scrollTop;
  });
}

async function copyAll() {
  try {
    await navigator.clipboard.writeText(props.preview.content);
    copied.value = true;
    copiedTimer = setTimeout(() => {
      copied.value = false;
    }, 1500);
  } catch {
    /* clipboard unavailable */
  }
}

watch([visibleCode, () => props.preview.language], highlight, { immediate: true });
watch(
  () => document.documentElement.dataset.theme,
  () => void highlight(),
);
onMounted(() => {
  const element = scroller.value;
  if (element) viewportHeight.value = element.clientHeight;
});
onBeforeUnmount(() => {
  if (scrollFrame) cancelAnimationFrame(scrollFrame);
  if (copiedTimer) clearTimeout(copiedTimer);
});
</script>

<template>
  <div class="text-preview">
    <!-- FCode 同款 chrome：macOS 三点 + 语言/编码 + 复制 -->
    <section class="f-code">
      <header class="f-code-header">
        <div class="f-code-dots">
          <span class="f-code-dot f-code-dot-red" aria-hidden="true"></span>
          <span class="f-code-dot f-code-dot-yellow" aria-hidden="true"></span>
          <span class="f-code-dot f-code-dot-green" aria-hidden="true"></span>
        </div>
        <span
          v-if="preview.language && preview.language !== 'text'"
          class="f-code-lang"
        >{{ preview.language }}</span>
        <span class="preview-meta">{{ preview.encoding }} · {{ lines.length }} {{ $t("package.textLines") }}
          <span v-if="preview.truncated"> · {{ $t("package.previewTruncated") }}</span>
        </span>
        <button type="button" class="preview-copy" @click="copyAll">
          <svg v-if="copied" viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12 5 5L20 7" /></svg>
          <svg v-else viewBox="0 0 24 24" aria-hidden="true"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V6a1 1 0 0 1 1-1h9" /></svg>
          <span>{{ copied ? $t("package.copied") : $t("package.copy") }}</span>
        </button>
      </header>
      <div class="text-preview-code" @scroll.passive="onScroll">
        <div class="text-preview-spacer" :style="{ height: `${totalHeight}px` }">
          <div class="text-preview-offset" :style="offsetStyle">
            <div
              v-if="highlighted"
              class="text-preview-shiki"
              v-html="highlighted"
            ></div>
            <pre v-else class="text-preview-plain"><code>{{ visibleCode }}</code></pre>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.text-preview {
  display: grid;
  min-width: 0;
}
/* FCode 同款卡片 chrome（与 ui/FCode.vue 一致）。 */
.f-code {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
}
.f-code-header {
  align-items: center;
  display: flex;
  gap: 10px;
  padding: 8px 12px;
}
.f-code-dots {
  display: flex;
  gap: 7px;
}
.f-code-dot {
  border: 0;
  border-radius: 50%;
  height: 11px;
  width: 11px;
}
.f-code-dot-red {
  background: #ff5f57;
}
.f-code-dot-yellow {
  background: #febc2e;
}
.f-code-dot-green {
  background: #28c840;
}
.f-code-lang {
  color: var(--muted-foreground);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 11px;
  margin-inline-start: auto;
}
.preview-meta {
  color: var(--muted-foreground);
  font-size: 11px;
}
.preview-copy {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font-size: 11px;
  font-weight: 650;
  gap: 5px;
  padding: 4px 7px;
  transition: background-color 120ms ease, color 120ms ease;
}
.preview-copy:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.preview-copy svg {
  fill: none;
  height: 13px;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
  width: 13px;
}
.text-preview-code {
  border-top: 1px solid var(--border);
  max-height: 420px;
  min-height: 260px;
  overflow: auto;
}
.text-preview-spacer {
  min-width: max-content;
  position: relative;
}
.text-preview-offset {
  will-change: transform;
}
.text-preview-plain {
  margin: 0;
  padding: 14px 16px;
}
.text-preview-plain code,
.text-preview-shiki :deep(code) {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12.5px;
  line-height: 20px;
  white-space: pre;
}
.text-preview-shiki :deep(pre) {
  background: transparent !important;
  margin: 0;
  padding: 14px 16px;
}
</style>
