<script setup lang="ts">
import { computed, onBeforeUnmount, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { isTauri, tauriApi } from "@/api";
import type { LotEditorSession } from "@/api/tauri";
import { useGamePackagesStore } from "@/stores/gamePackages";
import type { RasterSource } from "@/lib/raster-editor/schema";
import PropertyEditorViewport from "@/pages/packages/components/property-editor/PropertyEditorViewport.vue";
const props = defineProps<{
  raw: string;
  png: string;
  width: number;
  height: number;
  metersPerPixel: number;
  colors: [number, number, number, number][];
  target: RasterSource | null;
  texture: boolean;
}>();
const { t } = useI18n();
const packages = useGamePackagesStore();
const session = shallowRef<LotEditorSession | null>(null);
const error = shallowRef("");
const loading = shallowRef(false);
let generation = 0;
onBeforeUnmount(() => generation++);
watch(
  () =>
    [props.target, packages.opened.map((p) => p.package.packageId)] as const,
  async () => {
    const epoch = ++generation;
    session.value = null;
    error.value = "";
    if (!isTauri()) return;
    loading.value = true;
    try {
      let target = props.target;
      // New masks still need the game's shared surface and normal atlases.
      if (!target?.tgi || target.packageId == null) {
        for (const p of packages.opened) {
          const docs = await tauriApi.raster.listPropertyDocuments(
            p.package.packageId,
            "lot",
          );
          if (epoch !== generation) return;
          if (docs[0]) {
            target = {
              name: "",
              packageId: p.package.packageId,
              tgi: docs[0].tgi,
            };
            break;
          }
        }
      }
      if (target?.tgi && target.packageId != null) {
        const value = await tauriApi.packages.readLotEditorSession(
          target.packageId,
          target.tgi,
        );
        if (epoch === generation) session.value = value;
      }
    } catch (e) {
      if (epoch === generation) error.value = String(e);
    } finally {
      if (epoch === generation) loading.value = false;
    }
  },
  { immediate: true, deep: true },
);
const pngUrl = (value?: string | null) =>
  value ? `data:image/png;base64,${value}` : null;
const bound = computed(() => (props.target ? session.value : null));
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
</script>
<template>
  <div class="lot-preview">
    <p v-if="loading" role="status">{{ t("rasterEditor.loadingPreview") }}</p>
    <p v-else-if="error" role="alert">
      {{ t("rasterEditor.previewError") }} {{ error }}
    </p>
    <p v-else-if="!session?.lotSurfacePng" role="status">
      {{ t("rasterEditor.missingAtlas") }}
    </p>
    <PropertyEditorViewport
      v-if="session?.lotSurfacePng"
      :model-payload="null"
      :model-lods="[]"
      :active-lod="0"
      render-mode="refined"
      :grouping="grouping"
      :lot-size="
        (texture ? session?.lotSize : bound?.lotSize) ?? [
          width * metersPerPixel,
          height * metersPerPixel,
        ]
      "
      :lot-tile-period="bound?.lotTilePeriod ?? [6, 6]"
      :lot-placement="null"
      :lot-colors="colors"
      :lot-colors-authored="[true, true, true, true]"
      :lot-border-colors="bound?.lotBorderColors ?? []"
      :lot-border-widths="bound?.lotBorderWidths ?? [0, 0, 0, 0]"
      :lot-border-pattern-indices="
        bound?.lotBorderPatternIndices ?? [0, 0, 0, 0]
      "
      :lot-color-heights="bound?.lotColorHeights"
      :lot-border-heights="bound?.lotBorderHeights"
      :lot-base-tile="bound?.lotBaseTile ?? 8"
      :lot-overlay-box-offset="null"
      :lot-model-bbox-center="null"
      :lot-mask-png="texture ? pngUrl(session?.lotMaskPng) : png"
      :lot-mask-raw-rgba="texture ? (session?.lotMaskRawRgba ?? null) : raw"
      :lot-albedo-png="null"
      :lot-surface-png="texture ? png : pngUrl(session?.lotSurfacePng)"
      :lot-normal-atlas-png="pngUrl(session?.lotNormalAtlasPng)"
      :prop-models="emptyMap"
      :prop-tree-ids="treeIds"
      :tree-model-payloads="[]"
      :tree-atlas-png="null"
      :decal-textures="[]"
      :selected-id="null"
      :hidden-units="emptySet"
      :group-visibility="{ lot: true }"
      model-state="missing"
    />
  </div>
</template>
<style scoped>
.lot-preview {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.lot-preview :deep(.viewport-pane) {
  flex: 1;
  min-height: 0;
}
p {
  font-size: 11px;
  color: var(--muted-foreground);
  margin: 4px 0;
}
</style>
