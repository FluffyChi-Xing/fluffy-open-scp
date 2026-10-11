<script setup lang="ts">
import { ref, watch, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import type { TerrainField } from "@/lib/map-graph";
const props = defineProps<{
  field?: TerrainField;
  channel?: "grass" | "trees";
  radius?: number;
  strength?: number;
}>();
const emit = defineEmits<{
  stroke: [value: { x: number; y: number; radius: number; strength: number }];
}>();
const { t } = useI18n();
const canvas = ref<HTMLCanvasElement>();
function draw() {
  const ctx = canvas.value?.getContext("2d");
  if (!ctx) return;
  ctx.clearRect(0, 0, 128, 128);
  if (!props.field) return;
  const pixels = ctx.createImageData(128, 128);
  for (let i = 0; i < 16384; i++) {
    const value = props.field.heights[i]! * 255,
      mask = props.channel ? props.field[props.channel][i]! / 255 : 0;
    pixels.data.set(
      [
        value * (1 - mask),
        value * (1 - mask) + 190 * mask,
        value * (1 - mask),
        255,
      ],
      i * 4,
    );
  }
  ctx.putImageData(pixels, 0, 0);
}
function paint(event: PointerEvent) {
  if (!props.channel || !props.field || !(event.buttons & 1)) return;
  canvas.value!.setPointerCapture(event.pointerId);
  const box = canvas.value!.getBoundingClientRect();
  emit("stroke", {
    x: ((event.clientX - box.left) / box.width) * 128,
    y: ((event.clientY - box.top) / box.height) * 128,
    radius: Math.max(1, Math.min(32, props.radius || 8)),
    strength: Math.max(0, Math.min(1, props.strength ?? 0.7)),
  });
}
watch(() => [props.field, props.channel], draw);
onMounted(draw);
</script>
<template>
  <canvas
    ref="canvas"
    width="128"
    height="128"
    class="nodrag nowheel nopan"
    :aria-label="t(channel ? 'mapGraph.paintHint' : 'mapGraph.fieldPreview')"
    @pointerdown="paint"
    @pointermove="paint"
  />
</template>
<style scoped>
canvas {
  width: 100%;
  aspect-ratio: 1;
  image-rendering: pixelated;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 6px;
  touch-action: none;
}
</style>
