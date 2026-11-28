<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import FButton from "@/components/ui/FButton.vue";
import type { EditorPackageScope } from "@/composables/useEditorPackages";
const props = defineProps<{ scope: EditorPackageScope }>();
const { t } = useI18n();
async function choose() {
  const path = await open({
    directory: true,
    title: t("editorPackages.directory"),
  });
  if (typeof path === "string") await props.scope.load(path);
}
</script>
<template>
  <section
    class="editor-packages"
    :aria-label="t('editorPackages.title')"
    :aria-busy="scope.loading"
  >
    <div class="editor-packages-controls">
      <span class="editor-packages-path" :title="scope.directory">{{
        scope.directory || t("editorPackages.unconfigured")
      }}</span>
      <span role="status">{{
        scope.loading
          ? t("editorPackages.loading")
          : t("editorPackages.loaded", { count: scope.opened.length })
      }}</span>
      <FButton
        size="sm"
        variant="secondary"
        :disabled="scope.loading"
        @click="choose"
        >{{ t("editorPackages.choose") }}</FButton
      >
      <FButton
        size="sm"
        variant="ghost"
        :disabled="scope.loading"
        @click="scope.load(scope.directory || undefined)"
        >{{ t("editorPackages.retry") }}</FButton
      >
    </div>
    <p v-if="scope.failed" role="status">{{ t("editorPackages.failed") }}</p>
    <p v-if="scope.missing.length" role="status">
      {{ t("editorPackages.missing", { names: scope.missing.join(", ") }) }}
    </p>
  </section>
</template>
<style scoped>
.editor-packages {
  padding: 0.5rem 0.75rem;
  border-block-end: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 0.75rem;
}
.editor-packages-controls {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
}
.editor-packages-path {
  flex: 1;
  min-inline-size: 10rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
p {
  margin: 0.4rem 0 0;
  overflow-wrap: anywhere;
}
</style>
