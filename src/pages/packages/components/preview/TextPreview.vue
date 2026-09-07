<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { codeToHtml } from "shiki";
import type { TextPreview } from "@/api/tauri";

/**
 * 长文本不截断预览：行级虚拟滚动（只渲染可视窗口）+ shiki 仅高亮可视片段。
 * 之前整段内容喂给 shiki，资源一大（shader 容器 1MB+）就会卡死渲染。
 */
const props = defineProps<{ preview: TextPreview }>();

const LINE_HEIGHT = 20;
const OVERSCAN = 24;

const { t } = useI18n();
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
    <div class="preview-meta">
      <span>{{ preview.encoding }}</span
      ><span v-if="preview.language && preview.language !== 'text'">{{
        preview.language
      }}</span
      ><span>{{ lines.length }} {{ $t("package.textLines") }}</span
      ><span v-if="preview.truncated" class="preview-warn">{{
        $t("package.previewTruncated")
      }}</span
      ><button type="button" class="preview-copy" @click="copyAll">
        {{ copied ? $t("package.copied") : $t("package.copy") }}
      </button>
    </div>
    <div ref="scroller" class="text-preview-code" @scroll.passive="onScroll">
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
  </div>
</template>

<style scoped>
.text-preview {
  display: grid;
  gap: 10px;
  min-width: 0;
}
.preview-meta {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex-wrap: wrap;
  font-size: 11px;
  gap: 10px;
}
.preview-meta span {
  color: var(--muted-foreground);
}
.preview-warn {
  color: var(--warning) !important;
}
.preview-copy {
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  margin-inline-start: auto;
  padding: 2px 8px;
}
.preview-copy:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.text-preview-code {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
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
  padding: 0 16px;
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
  padding: 0 16px;
}
</style>
