<script setup lang="ts">
import FCode from "@/components/ui/FCode.vue";
import type { TextPreview } from "@/api/tauri";

defineProps<{ preview: TextPreview }>();
</script>

<template>
  <div class="text-preview">
    <div class="preview-meta">
      <span>{{ preview.encoding }}</span
      ><span v-if="preview.language && preview.language !== 'text'">{{
        preview.language
      }}</span
      ><span v-if="preview.truncated">{{
        $t("package.previewTruncated")
      }}</span>
    </div>
    <div class="text-preview-code">
      <FCode
        :code="preview.content"
        :lang="preview.language"
        :copy-label="$t('package.copy')"
        :copied-label="$t('package.copied')"
      />
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
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  gap: 10px;
}
.preview-meta span + span {
  color: var(--warning);
}
.text-preview-code {
  max-height: 420px;
  min-height: 260px;
  overflow: auto;
}
</style>
