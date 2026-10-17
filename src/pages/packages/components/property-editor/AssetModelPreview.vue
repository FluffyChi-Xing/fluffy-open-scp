<script setup lang="ts">
import { onBeforeUnmount, onMounted, shallowRef, watch } from "vue";
import type { LotModelPayload } from "@/api/tauri";
import { ThreeViewer, disposeObject } from "@/lib/three-viewer";
import SchemaPropertyViewport from "./SchemaPropertyViewport.vue";
const props = defineProps<{
  payload?: LotModelPayload | null;
  glb?: ArrayBuffer | null;
}>();
const host = shallowRef<HTMLElement | null>(null);
const error = shallowRef("");
let viewer: ThreeViewer | null = null,
  generation = 0,
  disposed = false;
async function render() {
  const epoch = ++generation;
  if (!props.glb) {
    viewer?.clearGroup("model");
    viewer?.invalidate();
    return;
  }
  if (!host.value) return;
  error.value = "";
  try {
    if (!viewer) {
      const created = await ThreeViewer.create(host.value, {
        maxPixelRatio: 2,
        interactionPixelRatio: 1,
      });
      if (disposed || epoch !== generation) {
        created.dispose();
        return;
      }
      viewer = created;
    }
    const { GLTFLoader } =
      await import("three/examples/jsm/loaders/GLTFLoader.js");
    const { scene } = await new GLTFLoader().parseAsync(props.glb, "");
    if (disposed || epoch !== generation) {
      disposeObject(scene);
      return;
    }
    // Imported glTF is Y-up; ThreeViewer's native asset world is Z-up.
    scene.rotation.x = Math.PI / 2;
    viewer.setModel([scene]);
    viewer.invalidate();
  } catch (cause) {
    if (epoch === generation) error.value = String(cause);
  }
}
watch(() => props.glb, render, { flush: "post" });
onMounted(render);
onBeforeUnmount(() => {
  disposed = true;
  generation++;
  viewer?.dispose();
});
</script>
<template>
  <div class="asset-preview nodrag nowheel nopan">
    <SchemaPropertyViewport v-if="payload" :model-payload="payload" />
    <div v-show="!payload" ref="host" class="glb-preview" />
    <p v-if="error" role="alert">{{ error }}</p>
    <p v-else-if="!payload && !glb" class="empty">
      上传模型或选择原生包中的模型以预览
    </p>
  </div>
</template>
<style scoped>
.asset-preview {
  height: 300px;
  position: relative;
  background: #182338;
  overflow: hidden;
  border-radius: 6px;
}
.asset-preview :deep(.viewport-pane) {
  height: 100%;
}
.asset-preview :deep(.viewport-visibility) {
  display: none;
}
.glb-preview {
  position: absolute;
  inset: 0;
}
.empty {
  position: absolute;
  inset: 40% 10% auto;
  color: #b5c4d9;
  text-align: center;
  font-size: 12px;
}
.asset-preview > p[role="alert"] {
  position: absolute;
  inset: auto 8px 8px;
  color: #f87171;
  font-size: 12px;
}
</style>
