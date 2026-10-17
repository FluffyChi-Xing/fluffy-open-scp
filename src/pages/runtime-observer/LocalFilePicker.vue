<script setup lang="ts">
import { shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import FButton from "@/components/ui/FButton.vue";
defineProps<{ label: string; accept: string; disabled?: boolean }>();
const emit = defineEmits<{ select: [file: File] }>();
const { t } = useI18n();
const input = shallowRef<HTMLInputElement>();
const filename = shallowRef("");
function pick(event: Event) {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (file) {
    filename.value = file.name;
    emit("select", file);
  }
  target.value = "";
}
</script>
<template>
  <div class="file-picker">
    <span>{{ label }}</span>
    <input
      ref="input"
      type="file"
      hidden
      :accept="accept"
      :disabled="disabled"
      :aria-label="label"
      @change="pick"
    />
    <FButton
      variant="secondary"
      :disabled="disabled"
      :aria-label="`${t('diagnostics.chooseFile')} · ${label}`"
      @click="input?.click()"
      >{{ t("diagnostics.chooseFile") }}</FButton
    >
    <span class="filename" :title="filename" aria-live="polite">{{
      filename || t("diagnostics.noFile")
    }}</span>
  </div>
</template>
<style scoped>
.file-picker {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  min-width: 0;
  font-size: 12px;
  color: var(--muted-foreground);
}
.filename {
  max-width: 240px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-picker :deep(.f-button) {
  min-height: 28px;
  font-size: 12px;
  font-weight: 500;
  padding: 0 9px;
}
</style>
