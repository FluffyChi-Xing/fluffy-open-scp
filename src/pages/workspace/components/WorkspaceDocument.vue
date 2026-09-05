<script setup lang="ts">
import { shallowRef, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FMarkdown from "@/components/markdown/FMarkdown.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FTextarea from "@/components/ui/FTextarea.vue";
import type { MarkdownDocument } from "@/api";

interface Props {
  document: MarkdownDocument | null;
  saving: boolean;
  error: string;
  selectTitle: string;
  selectDescription: string;
}
const props = defineProps<Props>();
const emit = defineEmits<{ save: [content: string] }>();
const draft = shallowRef("");
const editing = shallowRef(true);
watch(() => props.document, syncDraft, { immediate: true });
function syncDraft(document: MarkdownDocument | null) {
  draft.value = document?.content ?? "";
}
function selectMode() {
  editing.value = !editing.value;
}
function save() {
  emit("save", draft.value);
}
</script>

<template>
  <section class="workspace-document" aria-live="polite">
    <template v-if="props.document">
      <header class="document-header">
        <div class="document-title">
          <FTypography :header="3" spacing="none">{{
            props.document.relativePath
          }}</FTypography
          ><FTypography paragraphy type="secondary" spacing="none"
            >{{ $t("workspace.revision") }}
            {{ props.document.revision.slice(0, 10) }}</FTypography
          >
        </div>
        <div class="document-actions">
          <button class="quiet-button" type="button" @click="selectMode">
            <FIcon
              :name="editing ? 'Eye' : 'Pencil'"
              :size="16"
              aria-label=""
            />{{
              editing ? $t("workspace.preview") : $t("workspace.edit")
            }}</button
          ><button
            v-if="editing"
            class="primary-button"
            type="button"
            :disabled="props.saving"
            @click="save"
          >
            <FIcon name="Save" :size="16" aria-label="" />{{
              props.saving ? $t("common.loading") : $t("workspace.save")
            }}
          </button>
        </div>
      </header>
      <div class="document-content">
        <FTextarea
          v-if="editing"
          v-model="draft"
          :rows="18"
          :aria-label="$t('workspace.markdownLabel')"
        /><FMarkdown v-else :source="draft" />
      </div>
      <p v-if="props.error" class="error-message" role="alert">
        {{ props.error }}
      </p>
    </template>
    <div v-else class="document-empty">
      <FIcon name="FileText" :size="24" aria-label="" /><FTypography
        :header="3"
        >{{ props.selectTitle }}</FTypography
      ><FTypography paragraphy type="secondary">{{
        props.selectDescription
      }}</FTypography>
    </div>
  </section>
</template>

<style scoped>
.workspace-document {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  min-width: 0;
  padding: 22px;
}
.document-header {
  align-items: flex-start;
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 16px;
  justify-content: space-between;
  padding-bottom: 15px;
}
.document-title {
  min-width: 0;
}
.document-title :deep(.f-typography-h3) {
  overflow-wrap: anywhere;
}
.document-actions {
  display: flex;
  flex: none;
  gap: 6px;
}
.primary-button,
.quiet-button {
  align-items: center;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  font-weight: 700;
  gap: 7px;
  justify-content: center;
  min-height: 36px;
  padding: 0 12px;
  white-space: nowrap;
}
.primary-button {
  background: var(--primary);
  border: 1px solid var(--primary);
  color: var(--primary-foreground);
}
.quiet-button {
  background: transparent;
  border: 1px solid transparent;
  color: var(--muted-foreground);
}
.primary-button:disabled {
  cursor: wait;
  opacity: 0.6;
}
.document-content {
  flex: 1;
  padding-top: 20px;
}
.document-content :deep(textarea) {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: var(--foreground);
  font:
    13px/1.7 ui-monospace,
    SFMono-Regular,
    Menlo,
    Consolas,
    monospace;
  min-height: 420px;
  padding: 16px;
  resize: vertical;
  width: 100%;
}
.document-content :deep(textarea:focus-visible) {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.document-content :deep(.f-markdown) {
  max-width: 720px;
}
.document-empty {
  align-items: center;
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
  min-height: 360px;
  text-align: center;
}
.error-message {
  color: var(--danger);
  font-size: 12px;
  margin: 12px 0 0;
}
@media (max-width: 600px) {
  .document-header {
    flex-direction: column;
  }
  .document-actions {
    width: 100%;
  }
  .document-actions button {
    flex: 1;
  }
}
</style>
