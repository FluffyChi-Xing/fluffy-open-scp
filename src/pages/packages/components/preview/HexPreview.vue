<script setup lang="ts">
import { computed } from "vue";
import type { PreviewData } from "@/api/tauri";

const props = defineProps<{ preview: PreviewData }>();
const rows = computed(() => {
  const result: Array<{ offset: number; hex: string; ascii: string }> = [];
  for (let index = 0; index < props.preview.bytes.length; index += 16) {
    const bytes = props.preview.bytes.slice(index, index + 16);
    result.push({
      offset: props.preview.offset + index,
      hex: bytes
        .map((value) => value.toString(16).padStart(2, "0"))
        .join(" ")
        .padEnd(47, " "),
      ascii: bytes
        .map((value) =>
          value >= 32 && value <= 126 ? String.fromCharCode(value) : ".",
        )
        .join(""),
    });
  }
  return result;
});
function offsetLabel(value: number) {
  return value.toString(16).padStart(8, "0").toUpperCase();
}
</script>

<template>
  <div class="hex-preview">
    <div class="preview-meta">
      <span>0x{{ offsetLabel(preview.offset) }}</span
      ><span
        >{{ preview.bytes.length.toLocaleString() }} /
        {{ preview.totalLength.toLocaleString() }} B</span
      >
    </div>
    <div class="hex-scroll" role="table" :aria-label="$t('package.hexPreview')">
      <div v-for="row in rows" :key="row.offset" class="hex-row" role="row">
        <code class="hex-offset">{{ offsetLabel(row.offset) }}</code
        ><code class="hex-bytes">{{ row.hex }}</code
        ><code class="hex-ascii">{{ row.ascii }}</code>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hex-preview {
  display: grid;
  gap: 10px;
  min-width: 0;
}
.preview-meta {
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  gap: 10px;
}
.hex-scroll {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  max-height: 380px;
  min-height: 260px;
  overflow: auto;
  padding: 12px;
}
.hex-row {
  column-gap: 14px;
  display: grid;
  grid-template-columns: 9ch 47ch 16ch;
  justify-content: start;
  line-height: 1.8;
  white-space: pre;
}
.hex-row + .hex-row {
  border-top: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
}
code {
  font:
    12px/1.8 ui-monospace,
    SFMono-Regular,
    Menlo,
    Consolas,
    monospace;
}
.hex-offset {
  color: var(--primary);
}
.hex-bytes {
  color: var(--foreground);
}
.hex-ascii {
  color: var(--muted-foreground);
}
</style>
