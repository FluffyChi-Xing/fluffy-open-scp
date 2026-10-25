<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { isTauri } from "@/api";
import { loadLotPreviewSession } from "@/components/raster/lotPreviewSession";
import { useGamePackagesStore } from "@/stores/gamePackages";
import type { LotEditorSession, Tgi } from "@/api/tauri";
import PropertyEditorViewport from "@/pages/packages/components/property-editor/PropertyEditorViewport.vue";

/**
 * Lot 分层示意图卡片（复用 raster 面板 Lot 预览链路）：loadLotPreviewSession
 * 绑定该 Lot 的会话数据（LotMask 四色量化地面/尺寸/边框带），以 ground-only
 * 视口渲染默认模式分层效果。视口随卡片进入可视区才挂载（IntersectionObserver）。
 */
const props = defineProps<{
  packageId: number;
  tgi: Tgi;
}>();

const { t } = useI18n();
const packages = useGamePackagesStore();
const root = ref<HTMLElement | null>(null);
const visible = ref(typeof IntersectionObserver === "undefined");
const session = shallowRef<LotEditorSession | null>(null);
const atlas = shallowRef<Partial<LotEditorSession> | null>(null);
const loading = ref(false);
const error = ref("");
let generation = 0;
let visibilityObserver: IntersectionObserver | null = null;

onMounted(() => {
  if (!root.value || typeof IntersectionObserver === "undefined") return;
  visibilityObserver = new IntersectionObserver(
    ([entry]) => {
      visible.value = entry.isIntersecting;
    },
    { root: root.value.closest(".lot-library"), threshold: 0.01 },
  );
  visibilityObserver.observe(root.value);
});
onBeforeUnmount(() => {
  generation += 1;
  visibilityObserver?.disconnect();
});

watch(
  () => [visible.value, props.packageId, props.tgi] as const,
  async () => {
    const epoch = ++generation;
    if (!visible.value || !isTauri()) return;
    loading.value = true;
    error.value = "";
    try {
      const value = await loadLotPreviewSession(
        { name: "", packageId: props.packageId, tgi: props.tgi },
        packages.opened.map((p) => p.package.packageId),
        () => epoch !== generation,
      );
      if (epoch !== generation) return;
      session.value = value.bound;
      atlas.value = value.atlas;
    } catch (cause) {
      if (epoch === generation) error.value = String(cause);
    } finally {
      if (epoch === generation) loading.value = false;
    }
  },
  { immediate: true },
);

const grouping = {
  lights: [],
  decals: [],
  props: [],
  effects: [],
  spawners: [],
  pathPoints: [],
};
const emptyMap = new Map();
const emptySet = new Set<string>();
const treeIds = new Set<number>();
const pngUrl = (value?: string | null) =>
  value ? `data:image/png;base64,${value}` : null;
</script>

<template>
  <div ref="root" class="lot-diagram">
    <p v-if="loading" role="status">{{ t("rasterEditor.loadingPreview") }}</p>
    <p v-else-if="error" role="alert">{{ t("rasterEditor.previewError") }} {{ error }}</p>
    <PropertyEditorViewport
      v-else-if="visible && session"
      ground-only
      :model-payload="null"
      :model-lods="[]"
      :active-lod="0"
      render-mode="default"
      :grouping="grouping"
      :lot-size="session.lotSize"
      :lot-tile-period="session.lotTilePeriod ?? [6, 6]"
      :lot-placement="null"
      :lot-colors="session.lotColors"
      :lot-colors-authored="[true, true, true, true]"
      :lot-border-colors="session.lotBorderColors ?? []"
      :lot-border-widths="session.lotBorderWidths ?? [0, 0, 0, 0]"
      :lot-border-pattern-indices="session.lotBorderPatternIndices ?? [0, 0, 0, 0]"
      :lot-color-heights="session.lotColorHeights"
      :lot-border-heights="session.lotBorderHeights"
      :lot-base-tile="session.lotBaseTile ?? 8"
      :lot-overlay-box-offset="null"
      :lot-model-bbox-center="null"
      :lot-mask-png="pngUrl(session.lotMaskPng)"
      :lot-mask-raw-rgba="session.lotMaskRawRgba ?? null"
      :lot-albedo-png="pngUrl(session.lotAlbedoPng ?? atlas?.lotAlbedoPng)"
      :lot-surface-png="null"
      :lot-normal-atlas-png="null"
      :prop-models="emptyMap"
      :prop-tree-ids="treeIds"
      :tree-model-payloads="[]"
      :tree-atlas-png="null"
      :decal-textures="[]"
      :selected-id="null"
      :hidden-units="emptySet"
      :group-visibility="{ lot: true, dimensions: false }"
      model-state="missing"
    />
    <p v-else role="status">{{ t("rasterEditor.loadingPreview") }}</p>
  </div>
</template>

<style scoped>
.lot-diagram {
  background: #10192b;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  height: 118px;
  min-height: 0;
  overflow: hidden;
  position: relative;
}
.lot-diagram :deep(.viewport-pane) {
  flex: 1;
  min-height: 0;
}
.lot-diagram p {
  color: var(--muted-foreground);
  font-size: 10.5px;
  margin: 0;
  padding: 8px;
  text-align: center;
}
</style>
