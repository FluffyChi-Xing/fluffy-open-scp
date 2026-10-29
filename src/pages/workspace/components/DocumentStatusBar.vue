<script setup lang="ts">
/**
 * 编辑器底部状态栏（仿 Obsidian）：左侧显示当前文档路径（无文档时回退
 * 工作区根路径），右侧显示脏标记与词数/字符数（口径见 document-stats.ts）。
 */
const props = defineProps<{
  path: string;
  rootPath: string;
  words: number;
  characters: number;
  dirty: boolean;
}>();
</script>

<template>
  <footer class="doc-status">
    <span class="status-path" :title="props.path || props.rootPath">{{
      props.path || props.rootPath
    }}</span>
    <span class="status-right">
      <span v-if="props.dirty" class="status-dirty">{{
        $t("workspace.dirty")
      }}</span>
      <span class="status-stat">{{
        $t("workspace.statusWords", { count: props.words })
      }}</span>
      <span class="status-sep" aria-hidden="true">·</span>
      <span class="status-stat">{{
        $t("workspace.statusCharacters", { count: props.characters })
      }}</span>
    </span>
  </footer>
</template>

<style scoped>
.doc-status {
  align-items: center;
  background: var(--surface);
  border-top: 1px solid var(--border);
  color: var(--muted-foreground);
  display: flex;
  flex: none;
  font-size: 11px;
  gap: 10px;
  justify-content: space-between;
  min-height: 28px;
  padding: 0 12px;
}
.status-path {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.status-right {
  align-items: center;
  display: inline-flex;
  flex: none;
  gap: 8px;
  white-space: nowrap;
}
.status-dirty {
  align-items: center;
  align-self: center;
  background: var(--accent);
  border-radius: 999px;
  color: var(--primary);
  display: inline-flex;
  font-weight: 650;
  padding: 1px 8px;
}
.status-sep {
  color: var(--subtle-foreground);
}
</style>
