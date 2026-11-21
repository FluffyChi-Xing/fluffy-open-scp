<script setup lang="ts">
// Parallel workbench adapter copied from frozen SchemaPropertyViewport.vue.
// Preserve its lot/visibility bindings; restore authoring metadata before
// grouping so dropped decals retain their unattached preview path.
import { computed, shallowRef, useAttrs } from "vue";
import PropertyEditorViewport from "@/pages/packages/components/property-editor/PropertyEditorViewport.vue";
import { unitsFromSchema } from "@/pages/packages/components/property-editor/peSchemaDoc";
import type { UnitGrouping } from "@/pages/packages/components/property-editor/usePropertyEditorSession";
import { restoreDecalSources } from "./workbenchDecals";

defineOptions({ inheritAttrs: false });
const props = defineProps<{ schema?: Record<string, unknown> | null }>();
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
        grouping.pathPoints.push(unit);
        break;
    }
  }
  const lot = props.schema.lot as Record<string, unknown>;
  const editor = props.schema.editor as {
    groups?: Record<string, boolean>;
    unitVisibility?: Record<string, boolean>;
  };
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
  return { ...defaults, ...normalizedAttrs, ...schemaBinding.value };
}
defineExpose({
  groundPointAt: (x: number, y: number) =>
    viewport.value?.groundPointAt(x, y) ?? null,
  captureRender: (options?: { includeDecals?: boolean }) =>
    viewport.value?.captureRender(options) ?? Promise.resolve(null),
});
</script>
<template>
  <PropertyEditorViewport ref="viewport" v-bind="binding()" />
</template>
