<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { LotUnitDto } from "@/api/tauri";
import PropertyEditorProperties from "./PropertyEditorProperties.vue";
import PropertyEditorTransformPanel from "./PropertyEditorTransformPanel.vue";
import PropertyEditorMetadataPanel from "./PropertyEditorMetadataPanel.vue";
import PropertyEditorRenderTelemetry from "./PropertyEditorRenderTelemetry.vue";
import { unitId } from "./unitGizmos";

/**
 * 右侧检查器：横向 tab 栏（属性 / 坐标 / 元数据 / 渲染遥测）。
 * 属性 = 原只读属性行表；坐标 = 变换编辑；元数据 = 按 Unit 类型的语义
 * 字段编辑。tab 事件上抛由壳落本地编辑命令。
 */
const props = defineProps<{
  unit: LotUnitDto | null;
  /** 手柄拖拽中的实时变换（坐标 tab 即时显示）。 */
  liveTransform?: { id: string; position: [number, number, number] } | null;
}>();
const emit = defineEmits<{
  "update-transform": [id: string, matrix: number[]];
  "update-fields": [id: string, patch: Record<string, unknown>];
}>();
const { t } = useI18n();

/** 仅当实时变换属于当前选中 unit 时下传。 */
const unitLive = computed(
  () =>
    props.liveTransform && props.unit && props.liveTransform.id === unitId(props.unit)
      ? { position: props.liveTransform.position }
      : null,
);

type TabId = "properties" | "transform" | "metadata" | "render";
const activeTab = ref<TabId>("properties");
const TABS: { id: TabId; icon: string; label: string }[] = [
  { id: "properties", icon: "List", label: "package.tabProperties" },
  { id: "transform", icon: "Crosshair", label: "package.tabTransform" },
  { id: "metadata", icon: "Settings2", label: "package.tabMetadata" },
  { id: "render", icon: "Activity", label: "package.tabRenderTelemetry" },
];
</script>

<template>
  <aside class="inspector" :aria-label="$t('package.inspector')">
    <nav class="inspector-tabs" role="tablist" :aria-label="$t('package.inspector')">
      <button
        v-for="tab in TABS"
        :key="tab.id"
        type="button"
        role="tab"
        class="inspector-tab"
        :class="{ active: activeTab === tab.id }"
        :aria-selected="activeTab === tab.id"
        :title="t(tab.label)"
        @click="activeTab = tab.id"
      >
        <FIcon :name="tab.icon" :size="15" aria-label="" />
        <span>{{ t(tab.label) }}</span>
      </button>
    </nav>
    <div class="inspector-content">
      <PropertyEditorProperties
        v-if="activeTab === 'properties'"
        :unit="unit"
      />
      <PropertyEditorTransformPanel
        v-else-if="activeTab === 'transform'"
        :unit="unit"
        :live="unitLive"
        @update-transform="(id, matrix) => emit('update-transform', id, matrix)"
      />
      <PropertyEditorMetadataPanel
        v-else-if="activeTab === 'metadata'"
        :unit="unit"
        @update-fields="(id, patch) => emit('update-fields', id, patch)"
      />
      <!-- 只读渲染遥测：不接任何视口 prop，切页签不会触发 rebuildScene()。 -->
      <PropertyEditorRenderTelemetry v-else />
    </div>
  </aside>
</template>

<style scoped>
.inspector {
  border-left: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
}
/* 横向 tab（低代码引擎检查器对齐）：文字+下划线激活态 */
.inspector-tabs {
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  gap: 2px;
  padding: 0 8px;
}
.inspector-tab {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  gap: 6px;
  padding: 8px 10px 7px;
}
.inspector-tab:hover {
  color: var(--foreground);
}
.inspector-tab.active {
  border-bottom-color: var(--accent);
  color: var(--accent);
}
.inspector-content {
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow-y: auto;
}
/* 属性面板自带左侧分隔线与滚动，嵌套进检查器后去掉避免双边框/双滚动 */
.inspector-content :deep(.properties) {
  border-inline-start: 0;
  overflow-y: visible;
}
</style>
