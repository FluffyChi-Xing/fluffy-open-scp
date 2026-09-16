<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ThreeViewer } from "@/lib/three-viewer";
import { parseObjModel } from "@/lib/three-obj";

const props = defineProps<{ objBase64s: string[] }>();
const { t } = useI18n();
const container = shallowRef<HTMLElement | null>(null);
const error = ref("");
const ready = ref(false);
const lightAzimuth = ref(45);
const lightElevation = ref(55);
const viewer = shallowRef<ThreeViewer | null>(null);

async function rebuild() {
  const instance = viewer.value;
  if (!instance || !props.objBase64s.length) return;
  ready.value = false;
  error.value = "";
  try {
    // 多网格（同一模型的多个 MESH 节）并行解析后合并进同一场景
    const objects = await Promise.all(props.objBase64s.map(parseObjModel));
    instance.setModel(objects);
    instance.setKeyLight(lightAzimuth.value, lightElevation.value);
    ready.value = true;
  } catch {
    error.value = t("package.meshLoadFailed");
  }
}
onMounted(async () => {
  const element = container.value;
  if (!element) return;
  try {
    viewer.value = await ThreeViewer.create(element);
    viewer.value.setKeyLight(lightAzimuth.value, lightElevation.value);
    await rebuild();
  } catch {
    error.value = t("package.meshLoadFailed");
  }
});
onBeforeUnmount(() => {
  viewer.value?.dispose();
  viewer.value = null;
});
watch([lightAzimuth, lightElevation], () => {
  viewer.value?.setKeyLight(lightAzimuth.value, lightElevation.value);
});
watch(
  () => props.objBase64s,
  () => void rebuild(),
  { deep: true },
);
</script>

<template>
  <div class="mesh-preview">
    <div class="mesh-controls" role="group" :aria-label="$t('package.lightControls')">
      <label>
        <span>{{ $t("package.lightAzimuth") }}</span>
        <input v-model.number="lightAzimuth" type="range" min="0" max="360" />
      </label>
      <label>
        <span>{{ $t("package.lightElevation") }}</span>
        <input v-model.number="lightElevation" type="range" min="5" max="175" />
      </label>
    </div>
    <div ref="container" class="mesh-viewport" />
    <p v-if="error" class="mesh-error" role="alert">{{ error }}</p>
  </div>
</template>

<style scoped>
.mesh-preview {
  display: grid;
  gap: 8px;
  min-width: 0;
}
.mesh-controls {
  display: grid;
  gap: 6px;
}
.mesh-controls label {
  align-items: center;
  display: grid;
  font-size: 11px;
  grid-template-columns: auto 160px;
  gap: 8px;
}
.mesh-controls span {
  color: var(--subtle-foreground);
}
.mesh-controls input[type="range"] {
  accent-color: var(--primary);
  width: 160px;
}
.mesh-viewport {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: grab;
  height: 280px;
  min-width: 0;
  overflow: hidden;
  touch-action: none;
}
.mesh-viewport:active {
  cursor: grabbing;
}
.mesh-viewport canvas {
  display: block;
}
.mesh-error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  padding: 0 16px;
}
</style>
