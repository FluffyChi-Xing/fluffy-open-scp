<script setup lang="ts">
import type { TextPreview } from "@/api/tauri";

defineProps<{ preview: TextPreview }>();
</script>

<template>
  <div class="text-preview">
    <div class="preview-meta">
      <span>{{ preview.encoding }}</span
      ><span v-if="preview.truncated">{{
        $t("package.previewTruncated")
      }}</span>
    </div>
    <pre>{{ preview.content }}</pre>
  </div>
</template>

<style scoped>
.text-preview {
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
.preview-meta span + span {
  color: var(--warning);
}
pre {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font:
    12px/1.65 ui-monospace,
    SFMono-Regular,
    Menlo,
    Consolas,
    monospace;
  margin: 0;
  max-height: 380px;
  min-height: 260px;
  overflow: auto;
  padding: 14px;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
