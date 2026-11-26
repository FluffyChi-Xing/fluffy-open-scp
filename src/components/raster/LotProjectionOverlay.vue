<script setup lang="ts">
import { shallowRef, useTemplateRef, watch, watchEffect } from "vue";
import type { Object3D } from "three";
import { cloneCachedLotModelObjects, parseLotModelObjects } from "@/lib/three-gltf";
import { disposeObject } from "@/lib/viewerObjectDisposal";
import { drawLotProjection, type LotModelProjection } from "./lotModelProjection";

const props = defineProps<{
  projection: LotModelProjection;
  size: [number, number];
  width: number;
  height: number;
}>();
const canvas = useTemplateRef<HTMLCanvasElement>("projectionCanvas");
const roots = shallowRef<Object3D[]>([]);

watch(() => props.projection.payload, async (payload, _, onCleanup) => {
  const cached = cloneCachedLotModelObjects(payload);
  if (cached) {
    roots.value = cached;
    return;
  }
  let stale = false;
  let loaded: Object3D[] = [];
  onCleanup(() => {
    stale = true;
    loaded.forEach(disposeObject);
  });
  roots.value = [];
  // Keep the model pieces that can be decoded even if another piece fails.
  const parts = await Promise.allSettled(payload.glbs.map((glb) =>
    parseLotModelObjects([glb], payload.upAxis, payload.upAxisDeclared),
  ));
  loaded = parts.flatMap((part) => part.status === "fulfilled" ? part.value : []);
  if (stale) loaded.forEach(disposeObject);
  else roots.value = loaded;
}, { immediate: true });

watchEffect(() => {
  const element = canvas.value;
  const context = element?.getContext("2d");
  if (!element || !context) return;
  drawLotProjection(context, roots.value, props.projection, props.size, element.width, element.height);
}, { flush: "post" });
</script>

<template>
  <canvas
    ref="projectionCanvas"
    class="lot-projection"
    :width="width"
    :height="height"
    aria-hidden="true"
  />
</template>

<style scoped>
.lot-projection {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
</style>
