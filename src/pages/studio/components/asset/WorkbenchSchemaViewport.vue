<script setup lang="ts">
// Parallel workbench adapter copied from frozen SchemaPropertyViewport.vue.
// Preserve its lot/visibility bindings; restore authoring metadata before
// grouping so dropped decals retain their unattached preview path.
import { computed, shallowRef, useAttrs } from "vue";
import { useI18n } from "vue-i18n";
import FStartupLoading from "@/components/ui/FStartupLoading.vue";
import PropertyEditorViewport from "@/pages/packages/components/property-editor/PropertyEditorViewport.vue";
import { unitsFromSchema } from "@/pages/packages/components/property-editor/peSchemaDoc";
import type { ModelState, UnitGrouping } from "@/pages/packages/components/property-editor/usePropertyEditorSession";
import type { PathPointLike } from "@/render/plan/path";
import { restoreDecalSources } from "./workbenchDecals";

defineOptions({ inheritAttrs: false });
const props = defineProps<{
  schema?: Record<string, unknown> | null;
  modelState?: ModelState;
}>();
const { t } = useI18n();
const showModelLoading = computed(() => props.modelState === "loading");
const attrs = useAttrs();
type ViewProps = InstanceType<typeof PropertyEditorViewport>["$props"];
const viewport = shallowRef<InstanceType<typeof PropertyEditorViewport> | null>(
  null,
);
const defaults: ViewProps = {
    modelPayload: null,
    modelLods: [],
    activeLod: 0,
    renderMode: "refined",
    grouping: {
      lights: [],
      decals: [],
      props: [],
      effects: [],
      spawners: [],
      pathPoints: [],
    },
    pathPairs: [],
    lotSize: null,
    lotTilePeriod: null,
    lotPlacement: null,
    lotColors: [],
    lotColorsAuthored: [],
    lotBorderColors: [],
    lotBorderWidths: [],
    lotBorderPatternIndices: [],
    lotBaseTile: 8,
    lotOverlayBoxOffset: null,
    lotModelBboxCenter: null,
    propModels: new Map(),
    propTreeIds: new Set(),
    treeModelPayloads: [],
    treeAtlasPng: null,
    lotMaskPng: null,
    lotMaskRawRgba: null,
    lotAlbedoPng: null,
    decalTextures: [],
    lotSurfacePng: null,
    lotNormalAtlasPng: null,
    selectedId: null,
    hiddenUnits: new Set(),
    groupVisibility: {},
    modelState: "ready",
  };
// Keep scene inputs stable when only selection/tool/event bindings change.
// Recreating colors/grouping in render causes the viewport's ground watcher
// to rebuild the entire scene on every unrelated parent update.
const schemaBinding = computed(() => {
  if (!props.schema) return {};
  const units = unitsFromSchema(props.schema);
  restoreDecalSources(props.schema, units);
  const editor = props.schema.editor as {
    groups?: Record<string, boolean>;
    unitVisibility?: Record<string, boolean>;
    workbench?: { source?: string; paths?: { id: string; pointIds: string[] }[] };
  };
  const paths = editor?.workbench?.paths;
  const authoredPaths = paths && (paths.length > 0 || editor.workbench?.source !== "native");
  type Memberships = NonNullable<PathPointLike["previewPathMemberships"]>;
  const membershipByPoint = new Map<string, Memberships[number][]>();
  for (const path of paths ?? []) {
    for (const [order, pointId] of path.pointIds.entries()) {
      let memberships = membershipByPoint.get(pointId);
      if (!memberships) membershipByPoint.set(pointId, (memberships = []));
      memberships.push({ id: path.id, order });
    }
  }
  const grouping: UnitGrouping = {
    lights: [],
    decals: [],
    props: [],
    effects: [],
    spawners: [],
    pathPoints: [],
  };
  for (const unit of units) {
    switch (unit.kind) {
      case "light":
        grouping.lights.push(unit);
        break;
      case "decal":
        grouping.decals.push(unit);
        break;
      case "prop":
        grouping.props.push(unit);
        break;
      case "effect":
        grouping.effects.push(unit);
        break;
      case "spawner":
        grouping.spawners.push(unit);
        break;
      case "pathPoint":
        if (authoredPaths) {
          const point = {
            ...unit,
            previewPathMemberships: membershipByPoint.get(`pathPoint:${unit.index}`) ?? [],
          };
          grouping.pathPoints.push(point);
        } else {
          grouping.pathPoints.push(unit);
        }
        break;
    }
  }
  const lot = props.schema.lot as Record<string, unknown>;
  return {
    grouping,
    lotSize: lot.size,
    lotTilePeriod: lot.tilePeriod,
    lotPlacement: lot.placement,
    lotBaseTile: lot.baseTile,
    lotColors: (lot.colors as { rgba: number[] }[]).map((c) => c.rgba),
    lotColorsAuthored: (lot.colors as { authored?: boolean }[]).map(
      (c) => c.authored ?? false,
    ),
    lotBorderColors: lot.borderColors,
    lotBorderWidths: lot.borderWidths,
    lotColorHeights: lot.colorHeights,
    lotBorderHeights: lot.borderHeights,
    lotBorderPatternIndices: lot.borderPatterns,
    lotOverlayBoxOffset: lot.overlayBoxOffset,
    lotModelBboxCenter: lot.modelBBoxCenter,
    groupVisibility: editor?.groups ?? {},
    hiddenUnits: new Set(
      Object.entries(editor?.unitVisibility ?? {})
        .filter(([, visible]) => !visible)
        .map(([id]) => id),
    ),
  } as ViewProps;
});

function binding() {
  // useAttrs is updated by Vue, but is deliberately not a reactive computed
  // dependency. Read it in render so changed props and listeners still flow.
  const normalizedAttrs = Object.fromEntries(
    Object.entries(attrs).map(([key, value]) => [
      key.replace(/-([a-z])/g, (_, c: string) => c.toUpperCase()),
      value,
    ]),
  );
  return {
    ...defaults,
    ...normalizedAttrs,
    ...schemaBinding.value,
    // The workbench supplies its own loading indicator for this presentation state.
    modelState: showModelLoading.value ? "pending" : props.modelState ?? defaults.modelState,
  };
}
defineExpose({
  groundPointAt: (x: number, y: number) =>
    viewport.value?.groundPointAt(x, y) ?? null,
  captureRender: (options?: { includeDecals?: boolean }) =>
    viewport.value?.captureRender(options) ?? Promise.resolve(null),
});
</script>
<template>
  <div class="workbench-viewport">
    <PropertyEditorViewport
      ref="viewport"
      v-bind="binding()"
      :inert="showModelLoading"
    />
    <FStartupLoading
      v-if="showModelLoading"
      class="workbench-model-loading"
      :label="t('common.loading')"
    />
  </div>
</template>

<style scoped>
.workbench-viewport {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  position: relative;
}
.workbench-model-loading {
  position: absolute;
  inset: 0;
  z-index: 3;
  overflow: hidden;
}
</style>
