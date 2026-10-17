<script setup lang="ts">
import { shallowRef, useAttrs } from "vue";
import PropertyEditorViewport from "./PropertyEditorViewport.vue";
import { unitsFromSchema } from "./peSchemaDoc";
import type { UnitGrouping } from "./usePropertyEditorSession";

defineOptions({ inheritAttrs: false });
const props = defineProps<{ schema?: Record<string, unknown> | null }>();
const attrs = useAttrs();
type ViewProps = InstanceType<typeof PropertyEditorViewport>["$props"];
const viewport = shallowRef<InstanceType<typeof PropertyEditorViewport> | null>(
  null,
);
function binding() {
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
  const normalizedAttrs = Object.fromEntries(
    Object.entries(attrs).map(([key, value]) => [
      key.replace(/-([a-z])/g, (_, c: string) => c.toUpperCase()),
      value,
    ]),
  );
  const base = { ...defaults, ...normalizedAttrs };
  if (!props.schema) return base;
  const units = unitsFromSchema(props.schema);
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
    ...base,
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
