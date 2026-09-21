<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import FCode from "@/components/ui/FCode.vue";
import { isTauri, tauriApi } from "@/api";
import { foldBinaryRuns } from "@/lib/text-decode";
import type { TextPreview } from "@/api/tauri";

/**
 * 文本预览：全量渲染，杜绝虚拟滚动窗口数学带来的截断类问题。
 * - ≤1MB：FCode 整篇 shiki 高亮；
 * - >1MB：原生 <pre> 完整渲染（浏览器原生滚动），关闭高亮保渲染流畅；
 * - 超 8MB 首段（read_resource_text 上限）：滚动接近尾部自动经
 *   read_resource_text_range 续读 2MB（utf-16le 与无句柄场景除外），
 *   追加分段按二进制段折叠口径处理。
 */
const props = defineProps<{ preview: TextPreview }>();

const CHUNK_BYTES = 2 * 1024 * 1024;
/** 高亮渲染的字符数上限（超出转纯文本整篇渲染）。 */
const FCODE_MAX_CHARS = 1_000_000;
/** 距已加载尾部多少字符触发续读。 */
const LOAD_MORE_CHARS = 40_000;

const appendedText = ref("");
const loadedBytes = ref(props.preview.loadedBytes ?? 0);
const loadingMore = ref(false);
/** utf-16le 需偶数对齐，暂不参与续读；无句柄/非 Tauri 亦无法续读。 */
const eof = computed(
  () =>
    !props.preview.truncated ||
    props.preview.loadedBytes == null ||
    props.preview.encoding === "utf-16le" ||
    !isTauri() ||
    !props.preview.packageId ||
    !props.preview.tgi,
);

const fullContent = computed(() => props.preview.content + appendedText.value);

const lines = computed(() =>
  fullContent.value.length ? fullContent.value.split("\n") : [],
);

const useHighlight = computed(
  () => fullContent.value.length <= FCODE_MAX_CHARS,
);

onBeforeUnmount(() => {
  if (copiedTimer) clearTimeout(copiedTimer);
});

const copied = ref(false);
let copiedTimer: ReturnType<typeof setTimeout> | undefined;

async function copyAll() {
  try {
    await navigator.clipboard.writeText(fullContent.value);
    copied.value = true;
    copiedTimer = setTimeout(() => {
      copied.value = false;
    }, 1500);
  } catch {
    /* clipboard unavailable */
  }
}

function onPlainScroll(event: Event) {
  const element = event.target as HTMLElement;
  const remaining = element.scrollHeight - element.scrollTop - element.clientHeight;
  if (!eof.value && !loadingMore.value && remaining < LOAD_MORE_CHARS) {
    void loadMore();
  }
}

/** 续读下一段源字节并增量解码（utf-8 分界偶发裂字按二进制段折叠）。 */
async function loadMore() {
  if (eof.value || loadingMore.value) return;
  if (!props.preview.packageId || !props.preview.tgi) return;
  loadingMore.value = true;
  try {
    const buffer = await tauriApi.packages.readResourceTextRange(
      props.preview.packageId,
      props.preview.tgi,
      loadedBytes.value,
      CHUNK_BYTES,
    );
    const chunk = new Uint8Array(buffer);
    if (!chunk.length) {
      eof.value = true;
      return;
    }
    loadedBytes.value += chunk.byteLength;
    if (chunk.byteLength < CHUNK_BYTES) eof.value = true;
    let text: string;
    try {
      text = new TextDecoder("utf-8", { fatal: true }).decode(chunk);
    } catch {
      text = new TextDecoder("utf-8").decode(chunk);
    }
    appendedText.value += foldBinaryRuns(text);
  } catch (cause) {
    console.warn("[text-preview] 续读失败", cause);
    eof.value = true;
  } finally {
    loadingMore.value = false;
  }
}
</script>

<template>
  <div class="text-preview">
    <div class="preview-meta">
      <span>{{ preview.encoding }}</span>
      <span
        v-if="preview.language && preview.language !== 'text'"
        >{{ preview.language }}</span
      >
      <span>{{ lines.length }} {{ $t("package.textLines") }}</span>
      <span v-if="!eof" :class="{ 'preview-warn': !loadingMore }">
        {{ loadingMore ? $t("common.loading") : $t("package.previewTruncated") }}
      </span>
      <button type="button" class="preview-copy" @click="copyAll">
        {{ copied ? $t("package.copied") : $t("package.copy") }}
      </button>
    </div>
    <FCode
      v-if="useHighlight"
      :code="fullContent"
      :lang="preview.language"
    />
    <div v-else class="text-plain-wrap" @scroll.passive="onPlainScroll">
      <pre class="text-plain"><code>{{ fullContent }}</code></pre>
    </div>
  </div>
</template>

<style scoped>
.text-preview {
  display: grid;
  gap: 8px;
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
.preview-warn {
  color: var(--warning) !important;
}
.preview-copy {
  align-items: center;
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
.text-plain-wrap {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  max-height: 480px;
  min-height: 260px;
  overflow: auto;
}
.text-plain {
  margin: 0;
  padding: 14px 16px;
}
.text-plain code {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12.5px;
  line-height: 1.6;
  white-space: pre;
}
</style>
