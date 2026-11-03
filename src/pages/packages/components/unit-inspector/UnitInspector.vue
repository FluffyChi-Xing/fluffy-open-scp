<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { LotUnitDto, Tgi } from "@/api/tauri";
import PropertyEditorProperties from "@/pages/packages/components/property-editor/PropertyEditorProperties.vue";
import PropertyEditorTransformPanel from "@/pages/packages/components/property-editor/PropertyEditorTransformPanel.vue";
import PropertyEditorMetadataPanel from "@/pages/packages/components/property-editor/PropertyEditorMetadataPanel.vue";
import PropertyEditorRenderTelemetry from "@/pages/packages/components/property-editor/PropertyEditorRenderTelemetry.vue";
import { unitId } from "@/pages/packages/components/property-editor/unitGizmos";
import SpawnerUnitPanel from "./SpawnerUnitPanel.vue";

/**
 * 检查器外壳（平行实现，不复用冻结的 PropertyEditorInspector 实例）。
 *
 * 冻结区的 PropertyEditorInspector.vue 不允许就地改造，这里以「组合」方式
 * 复刻同一套页签，并在「属性」页签顶部插入生成器（spawner）属性面板：
 * 选中生成器单元时先给出锚点语义 / 命名 / 校验，再往下是原始属性行表。
 * 其余三个页签仍是冻结目录里的原组件实例（只读引用，不修改）。
 */
const props = defineProps<{
  unit: LotUnitDto | null;
  /** 当前 lot 全部单元（生成器面板做重名/统计校验）。 */
  units?: readonly LotUnitDto[];
  /** 手柄拖拽中的实时变换（坐标页签即时显示）。 */
  liveTransform?: { id: string; position: [number, number, number] } | null;
  /** 选中 prop 是否显示「缩放倍率」编辑。 */
  scaleEditable?: boolean;
  /** 编辑模式解锁态（透传给元数据/生成器面板）。 */
  editEnabled?: boolean;
  /** 当前包；属性栏按选中 effect 解析 Swarm 详情。 */
  packageId?: number | null;
  /** 当前 lot 资源 TGI；effect 语义标签创建时记录来源。 */
  sourceTgi?: Tgi | null;
}>();
const emit = defineEmits<{
  "update-transform": [id: string, matrix: number[]];
  "update-fields": [id: string, patch: Record<string, unknown>];
}>();

const { t } = useI18n();

const unitLive = computed(
  () =>
    props.liveTransform &&
    props.unit &&
    props.liveTransform.id === unitId(props.unit)
      ? { position: props.liveTransform.position }
      : null,
);

/** 单元全集：未显式传入时退化为「只有当前单元」，面板仍可用但不做统计。 */
const allUnits = computed<readonly LotUnitDto[]>(() =>
  props.units ?? (props.unit ? [props.unit] : []),
);
const isSpawner = computed(() => props.unit?.kind === "spawner");

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
        <span v-if="tab.id === 'properties' && isSpawner" class="tab-dot" aria-hidden="true" />
      </button>
    </nav>
    <div class="inspector-content">
      <template v-if="activeTab === 'properties'">
        <SpawnerUnitPanel
          v-if="isSpawner"
          :unit="unit"
          :units="allUnits"
          :edit-enabled="editEnabled"
          @update-fields="(id, patch) => emit('update-fields', id, patch)"
        />
        <PropertyEditorProperties
          :unit="unit"
          :units="allUnits"
          :package-id="packageId"
          :source-tgi="sourceTgi ?? null"
        />
      </template>
      <PropertyEditorTransformPanel
        v-else-if="activeTab === 'transform'"
        :unit="unit"
        :live="unitLive"
        @update-transform="(id, matrix) => emit('update-transform', id, matrix)"
      />
      <PropertyEditorMetadataPanel
        v-else-if="activeTab === 'metadata'"
        :unit="unit"
        :scale-editable="scaleEditable"
        :edit-enabled="editEnabled"
        @update-fields="(id, patch) => emit('update-fields', id, patch)"
      />
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
.inspector-tabs {
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  flex-wrap: wrap;
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
  white-space: nowrap;
}
.inspector-tab:hover {
  color: var(--foreground);
}
.inspector-tab.active {
  border-bottom-color: var(--brand);
  color: var(--brand);
  font-weight: 600;
}
.tab-dot {
  background: var(--brand);
  border-radius: 999px;
  height: 5px;
  width: 5px;
}
.inspector-content {
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow-y: auto;
}
.inspector-content :deep(.properties) {
  border-inline-start: 0;
  overflow-y: visible;
}
</style>
