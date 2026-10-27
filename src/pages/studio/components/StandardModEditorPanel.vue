<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import MapSourceTool from "./mod-flow/MapSourceTool.vue";
import ModAssetWorkbench from "./asset/ModAssetWorkbench.vue";

/**
 * 标准 Mod 编辑容器：map 走地图源工具；assets 整栏交给
 * ModAssetWorkbench（Blender 式三栏：文件树 / 视口+蓝图 / 属性面板）。
 */
const props = defineProps<{
  project: string;
  kind: string;
  manifest: Record<string, unknown>;
}>();
const emit = defineEmits<{ focus: []; fullscreen: [] }>();
const { t } = useI18n();
const fullscreen = ref(false);
const mapPackagePath = ref("");
const mapGroup = ref("");

const editorTitle = computed(() => {
  if (props.kind === "map") return t("studio.code.mapPreview");
  if (props.kind === "assets") return t("studio.code.assetEditor");
  return t("studio.code.editorPanel");
});
</script>

<template>
  <section class="standard-editor" :class="{ fullscreen }" :aria-label="editorTitle">
    <header v-if="kind !== 'assets'" class="standard-editor-head">
      <div>
        <p class="eyebrow">{{ t("studio.code.standardWorkbench") }}</p>
        <h2>{{ editorTitle }}</h2>
      </div>
      <div class="editor-actions">
        <button type="button" class="editor-icon" :title="t('studio.code.focusEditor')" :aria-label="t('studio.code.focusEditor')" @click="emit('focus')"><FIcon name="PanelTop" :size="14" aria-label="" /></button>
        <button type="button" class="editor-icon" :title="t('studio.code.fullscreenEditor')" :aria-label="t('studio.code.fullscreenEditor')" @click="fullscreen = !fullscreen"><FIcon :name="fullscreen ? 'Minimize2' : 'Maximize2'" :size="14" aria-label="" /></button>
        <span class="editor-kind">{{ kind }}</span>
      </div>
    </header>

    <div v-if="kind === 'map'" class="editor-content map-content">
      <MapSourceTool v-model:package-path="mapPackagePath" v-model:group="mapGroup" :disabled="false" />
    </div>
    <div v-else-if="kind === 'assets'" class="editor-content asset-content">
      <ModAssetWorkbench :project="project" />
    </div>
    <div v-else class="editor-content editor-empty">
      {{ t("studio.code.editorPending") }}
    </div>
  </section>
</template>

<style scoped>
.standard-editor { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--surface); }
.standard-editor.fullscreen { inset: 12px; position: fixed; z-index: 90; border: 1px solid var(--border); border-radius: 8px; box-shadow: var(--shadow-xl); }
.standard-editor-head { display: flex; align-items: center; justify-content: space-between; padding: 12px 14px; border-bottom: 1px solid var(--border); flex: 0 0 auto; }
.standard-editor-head h2 { margin: 2px 0 0; font-size: 14px; font-weight: 600; }
.eyebrow { margin: 0; color: var(--muted-foreground); font-size: 10px; text-transform: uppercase; letter-spacing: .08em; }
.editor-kind { color: var(--muted-foreground); font: 11px var(--font-mono, monospace); }
.editor-actions { align-items: center; display: flex; gap: 5px; }
.editor-icon { align-items: center; background: transparent; border: 1px solid var(--border); border-radius: 5px; color: var(--muted-foreground); cursor: pointer; display: inline-flex; height: 25px; justify-content: center; width: 25px; }
.editor-icon:hover { background: var(--surface-hover); color: var(--foreground); }
.editor-content { min-height: 0; flex: 1; overflow: auto; padding: 14px; }
.map-content { padding: 14px; }
.map-content :deep(.map-source-tool) { min-height: 100%; }
.asset-content { align-items: stretch; display: flex; flex-direction: column; gap: 0; overflow: hidden; padding: 0; border-top: 1px solid var(--border); }
.asset-content :deep(.maw) { flex: 1; min-height: 0; width: 100%; }
.editor-description, .editor-empty { margin: 0; color: var(--muted-foreground); font-size: 12px; }
</style>
