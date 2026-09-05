<script setup lang="ts">
import FEmpty from "@/components/extensions/FEmpty.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import type { PackageFile } from "@/api/tauri";
import { extensionIconUrl } from "@/lib/resource-types";

interface Props {
  files: PackageFile[];
  selectedPath: string;
  loading: boolean;
}
const props = defineProps<Props>();
const emit = defineEmits<{ select: [file: PackageFile] }>();
</script>

<template>
  <section class="package-files panel">
    <FTypography :header="4" spacing="none"
      ><FIcon name="Package" :size="15" aria-label="" />{{
        $t("package.packageFiles")
      }}</FTypography
    >
    <div v-if="props.loading" class="loading-list">
      <FSkeleton v-for="index in 4" :key="index" height="46px" rounded />
    </div>
    <div v-else-if="props.files.length" class="file-list">
      <button
        v-for="file in props.files"
        :key="file.path"
        class="file-item"
        :class="{ active: file.path === props.selectedPath }"
        type="button"
        @click="emit('select', file)"
      >
        <img class="file-icon" :src="extensionIconUrl(file.name)" alt="" /><span
          ><strong>{{ file.name }}</strong
          ><small>{{ Math.round(file.size / 1_000_000) }} MB</small></span
        >
      </button>
    </div>
    <FEmpty
      v-else
      icon-name="Package"
      :title="$t('package.noPackagesInFolder')"
      :desc="$t('package.chooseFolderHint')"
      variant="compact"
    />
  </section>
</template>

<style scoped>
.panel {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  min-width: 0;
  padding: 16px;
}
.package-files h4 {
  align-items: center;
  display: flex;
  gap: 7px;
}
.package-files h4 svg {
  color: var(--primary);
}
.file-list {
  display: grid;
  gap: 5px;
  margin-top: 14px;
}
.file-item {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  gap: 9px;
  min-height: 52px;
  padding: 6px 9px;
  text-align: start;
  width: 100%;
}
.file-item:hover,
.file-item.active {
  background: var(--accent);
  color: var(--foreground);
}
.file-item.active {
  font-weight: 700;
}
.file-item > svg {
  color: var(--primary);
  flex: none;
}
.file-icon {
  flex: none;
  height: 16px;
  object-fit: contain;
  width: 16px;
}
.file-item span {
  display: grid;
  gap: 3px;
  min-width: 0;
}
.file-item strong {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-item small {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.loading-list {
  display: grid;
  gap: 8px;
  margin-top: 14px;
}
.package-files :deep(.f-empty) {
  min-height: 180px;
  width: 100%;
}
</style>
