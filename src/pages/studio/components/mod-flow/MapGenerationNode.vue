<script setup lang="ts">
import { computed, ref, shallowRef, watch, onBeforeUnmount } from "vue";
import { Handle, Position } from "@vue-flow/core";
import { useI18n } from "vue-i18n";
import { inputs, type MapGraphNode, type TerrainField } from "@/lib/map-graph";
import { terrainToRegion } from "@/lib/map-graph-preview";
import type { Region3DData } from "@/lib/region-map";
import MapPreviewPane from "@/components/map/MapPreviewPane.vue";
import MapFieldCanvas from "./MapFieldCanvas.vue";
const props = defineProps<{
  node: MapGraphNode;
  field?: TerrainField;
  error?: string;
}>();
const emit = defineEmits<{ update: [node: MapGraphNode]; remove: [] }>();
const { t } = useI18n(),
  fileError = ref("");
const region = shallowRef<Region3DData | null>(null);
let previewTimer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => props.field,
  (field) => {
    clearTimeout(previewTimer);
    if (!field || props.node.kind !== "output") {
      region.value = null;
      return;
    }
    previewTimer = setTimeout(() => {
      region.value = terrainToRegion(field);
    }, 160);
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  clearTimeout(previewTimer);
  request++;
});
const isBrush = computed(
  () => props.node.kind === "grass" || props.node.kind === "trees",
);
const limits: Record<string, [number, number]> = {extent: [2048, 32768], elevation: [1, 10000], water: [-1000, 10000], seed: [0, 2147483647], frequency: [0.01, 256], octaves: [1, 12], persistence: [0, 1], mix: [0, 1], radius: [1, 32], strength: [0, 1]};
function configure(key: string, event: Event) {
  const [min, max] = limits[key] ?? [-Infinity, Infinity];
  const input = event.target as HTMLInputElement;
  let value = Math.max(min, Math.min(max, Number(input.value)));
  if (!Number.isFinite(value)) return;
  if (key === "seed" || key === "octaves") value = Math.round(value);
  input.value = String(value);
  emit("update", {
    ...props.node,
    config: {
      ...props.node.config,
      [key]: value,
    },
  });
}
let request = 0;
watch(
  () => props.node.id,
  () => {
    request++;
  },
);
async function importImage(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const current = ++request,
    id = props.node.id,
    url = URL.createObjectURL(file);
  fileError.value = "";
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 128;
    const ctx = canvas.getContext("2d")!;
    ctx.drawImage(image, 0, 0, 128, 128);
    const data = ctx.getImageData(0, 0, 128, 128).data;
    const pixels = Array.from(
      { length: 16384 },
      (_, i) =>
        (data[i * 4]! * 0.2126 +
          data[i * 4 + 1]! * 0.7152 +
          data[i * 4 + 2]! * 0.0722) /
        255,
    );
    if (current === request && id === props.node.id)
      emit("update", { ...props.node, pixels });
  } catch {
    fileError.value = "imageError";
  } finally {
    URL.revokeObjectURL(url);
  }
}
</script>
<template>
  <article class="generation-node" :class="{ output: node.kind === 'output' }">
    <header>
      <strong>{{ t(`mapGraph.nodes.${node.kind}`) }}</strong
      ><button
        class="nodrag"
        :aria-label="t('mapGraph.remove')"
        @click="emit('remove')"
      >
        ×
      </button>
    </header>
    <div class="ports">
      <div v-for="(port, i) in inputs[node.kind]" :key="port" class="port">
        <Handle
          :id="port"
          type="target"
          :position="Position.Left"
          :style="{ top: 60 + i * 28 + 'px' }"
        />{{ t(`mapGraph.ports.${port}`) }}
      </div>
      <Handle
        v-if="node.kind !== 'output'"
        :id="node.kind === 'extent' ? 'extent' : 'terrain'"
        type="source"
        :position="Position.Right"
        :style="{ top: '60px' }"
      />
      <div v-if="node.kind !== 'output'" class="source">
        {{
          t(
            node.kind === "extent"
              ? "mapGraph.ports.extent"
              : "mapGraph.ports.terrain",
          )
        }}
      </div>
    </div>
    <div class="body nodrag nopan nowheel">
      <label v-for="(value, key) in node.config" :key="key"
        >{{ t(`mapGraph.fields.${key}`)
        }}<input
          type="number"
          :value="value"
          step="any"
          @change="configure(key, $event)"
      /></label>
      <label v-if="node.kind === 'image'"
        >{{ t("mapGraph.import")
        }}<input
          type="file"
          accept="image/png,image/jpeg,image/webp"
          @change="importImage"
      /></label>
      <p v-if="error || fileError" role="status">
        {{ t(`mapGraph.errors.${fileError || error}`) }}
      </p>
      <MapPreviewPane
        v-if="node.kind === 'output'"
        :data="region"
        :title="t('mapGraph.nodes.output')"
      />
      <MapFieldCanvas
        v-else-if="node.kind !== 'extent'"
        :field="field"
        :channel="isBrush ? (node.kind as 'grass' | 'trees') : undefined"
        :radius="node.config.radius"
        :strength="node.config.strength"
        @stroke="
          emit('update', {
            ...node,
            strokes: [...(node.strokes ?? []), $event],
          })
        "
      />
      <template v-if="isBrush"
        ><p>{{ t("mapGraph.paintHint") }}</p>
        <button @click="emit('update', { ...node, strokes: [] })">
          {{ t("mapGraph.clear") }}
        </button></template
      >
    </div>
  </article>
</template>
<style scoped>
/* Blender node-editor look: fixed gray-black palette, independent of the
   app theme. Shared tokens live in styles/flow-presentation.css. */
.generation-node {
  position: relative;
  width: 310px;
  border: 1px solid var(--flow-border, #141414);
  border-radius: 8px;
  background: var(--flow-node, #282828);
  color: var(--flow-text, #e6e6e6);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4);
}
.generation-node.output {
  width: 760px;
}
header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 36px;
  padding: 0 12px;
  background: var(--flow-head-process, #2e6b2e);
  border-radius: 7px 7px 0 0;
  color: #f2f2f2;
  cursor: grab;
  font-size: 13px;
}
.generation-node.output header {
  background: var(--flow-head-output, #7f3030);
}
header button {
  background: transparent;
  border: 0;
  color: rgba(255, 255, 255, 0.65);
  cursor: pointer;
  font-size: 14px;
  padding: 2px 4px;
}
header button:hover {
  color: #ffffff;
}
.ports {
  padding: 10px 14px;
  font-size: 11px;
  min-height: 40px;
  color: var(--flow-text-dim, #9b9b9b);
}
.port {
  height: 28px;
}
.source {
  position: absolute;
  top: 48px;
  right: 14px;
}
.body {
  padding: 12px;
  display: grid;
  gap: 10px;
}
label {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  font-size: 12px;
  align-items: center;
}
input,
button {
  min-width: 0;
  border: 1px solid var(--flow-field-border, #3a3a3a);
  background: var(--flow-field, #1a1a1a);
  color: var(--flow-text, #e6e6e6);
  border-radius: 4px;
  padding: 5px;
}
input:focus-visible {
  outline: 2px solid var(--flow-select, rgba(255, 255, 255, 0.92));
  outline-offset: 1px;
}
p {
  margin: 0;
  color: var(--flow-text-dim, #9b9b9b);
  font-size: 12px;
}
</style>
