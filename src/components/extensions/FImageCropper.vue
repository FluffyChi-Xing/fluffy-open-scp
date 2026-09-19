<script setup lang="ts">
/**
 * FImageCropper：把用户上传的图片裁切到游戏 UI 的固定尺寸。
 *
 * 用法：v-model:open 控制显隐，width/height 指定输出像素与宽高比
 * （槽位 icon 128×128、hover 大图 454×263），format 指定导出 MIME。
 * 交互：上传 → 拖动平移 / 滚轮或滑杆缩放 → 确认；裁切框固定居中，
 * 框外压暗。确认后在离屏 canvas 上按输出尺寸重采样，emit dataURL。
 */
import { computed, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    open: boolean;
    width: number;
    height: number;
    format?: "image/png" | "image/jpeg";
    title?: string;
  }>(),
  { format: "image/png", title: "裁切图片" },
);
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "cropped", dataUrl: string): void;
}>();

const FRAME_MAX = 360; // 裁切框在屏幕上的最长边（css px）
const frameW = computed(() =>
  props.width >= props.height ? FRAME_MAX : Math.round(FRAME_MAX * (props.width / props.height)),
);
const frameH = computed(() =>
  props.height > props.width ? FRAME_MAX : Math.round(FRAME_MAX * (props.height / props.width)),
);

const fileInput = ref<HTMLInputElement>();
const stage = ref<HTMLCanvasElement>();
const source = ref<HTMLImageElement | null>(null);
const zoom = ref(1);
const offset = ref({ x: 0, y: 0 });
const fileName = ref("");
let dragStart: { x: number; y: number; ox: number; oy: number } | null = null;

/** 源图 → 画布的适配缩放（contain），zoom 在其上叠加。 */
const baseScale = computed(() => {
  const img = source.value;
  if (!img) return 1;
  const cw = frameW.value;
  const ch = frameH.value;
  return Math.max(cw / img.naturalWidth, ch / img.naturalHeight) * 1.05;
});
const scale = computed(() => baseScale.value * zoom.value);

function reset(): void {
  zoom.value = 1;
  offset.value = { x: 0, y: 0 };
}

function onPick(event: Event): void {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  fileName.value = file.name;
  const url = URL.createObjectURL(file);
  const img = new Image();
  img.onload = () => {
    source.value = img;
    reset();
    draw();
    URL.revokeObjectURL(url);
  };
  img.src = url;
}

function draw(): void {
  const canvas = stage.value;
  const img = source.value;
  if (!canvas || !img) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const cw = frameW.value;
  const ch = frameH.value;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  // 画布比裁切框大一圈（PAD），平移时框外也有内容
  const pad = 60;
  const w = img.naturalWidth * scale.value;
  const h = img.naturalHeight * scale.value;
  const dx = cw / 2 - w / 2 + offset.value.x;
  const dy = ch / 2 - h / 2 + offset.value.y;
  ctx.fillStyle = "#0b0f14";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(img, dx - pad, dy - pad, w, h);
  // 框外压暗：四块半透明矩形
  ctx.fillStyle = "rgb(0 0 0 / 55%)";
  ctx.fillRect(0, 0, canvas.width, (canvas.height - ch) / 2);
  ctx.fillRect(0, (canvas.height + ch) / 2, canvas.width, (canvas.height - ch) / 2);
  ctx.fillRect(0, (canvas.height - ch) / 2, (canvas.width - cw) / 2, ch);
  ctx.fillRect((canvas.width + cw) / 2, (canvas.height - ch) / 2, (canvas.width - cw) / 2, ch);
  // 裁切框描边
  ctx.strokeStyle = "#4aa3ff";
  ctx.lineWidth = 1.5;
  ctx.strokeRect((canvas.width - cw) / 2, (canvas.height - ch) / 2, cw, ch);
}

function onPointerDown(event: PointerEvent): void {
  dragStart = { x: event.clientX, y: event.clientY, ox: offset.value.x, oy: offset.value.y };
  (event.target as HTMLElement).setPointerCapture(event.pointerId);
}
function onPointerMove(event: PointerEvent): void {
  if (!dragStart) return;
  offset.value = { x: dragStart.ox + (event.clientX - dragStart.x), y: dragStart.oy + (event.clientY - dragStart.y) };
  draw();
}
function onPointerUp(): void {
  dragStart = null;
}
function onWheel(event: WheelEvent): void {
  event.preventDefault();
  zoom.value = clamp(zoom.value * (event.deltaY < 0 ? 1.1 : 0.9), 0.2, 8);
  draw();
}
function onZoomInput(event: Event): void {
  zoom.value = Number((event.target as HTMLInputElement).value);
  draw();
}

function confirm(): void {
  const img = source.value;
  if (!img) return;
  const out = document.createElement("canvas");
  out.width = props.width;
  out.height = props.height;
  const ctx = out.getContext("2d");
  if (!ctx) return;
  ctx.imageSmoothingQuality = "high";
  // 画布中心坐标系下的源图左上角 → 输出框取源图对应区域
  const w = img.naturalWidth * scale.value;
  const h = img.naturalHeight * scale.value;
  const fw = frameW.value;
  const fh = frameH.value;
  const dx = fw / 2 - w / 2 + offset.value.x;
  const dy = fh / 2 - h / 2 + offset.value.y;
  ctx.drawImage(img, dx, dy, w, h, 0, 0, props.width, props.height);
  emit("cropped", out.toDataURL(props.format, 0.92));
  emit("update:open", false);
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      source.value = null;
      fileName.value = "";
      reset();
    }
  },
);

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(v, max));
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="f-cropper-mask" @click.self="emit('update:open', false)">
      <div class="f-cropper">
        <header class="cropper-head">
          <span>{{ title }}（{{ width }}×{{ height }}）</span>
          <button type="button" class="cropper-close" @click="emit('update:open', false)">×</button>
        </header>
        <div
          class="cropper-stage"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
          @pointercancel="onPointerUp"
          @wheel="onWheel"
        >
          <canvas ref="stage" class="cropper-canvas" :width="frameW + 120" :height="frameH + 120" />
          <p v-if="!source" class="cropper-hint">选择图片后拖动平移、滚轮缩放</p>
        </div>
        <div class="cropper-actions">
          <input ref="fileInput" type="file" accept="image/*" class="cropper-file" @change="onPick" />
          <button type="button" class="cropper-btn" @click="fileInput?.click()">
            {{ source ? "重新选择" : "选择图片" }}
          </button>
          <label class="cropper-zoom">
            缩放
            <input
              type="range"
              min="0.2"
              max="8"
              step="0.01"
              :value="zoom"
              @input="onZoomInput"
            />
          </label>
          <button type="button" class="cropper-btn" :disabled="!source" @click="reset">重置</button>
          <button type="button" class="cropper-btn primary" :disabled="!source" @click="confirm">
            确认裁切
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.f-cropper-mask {
  align-items: center;
  background: rgb(0 0 0 / 60%);
  display: flex;
  inset: 0;
  justify-content: center;
  position: fixed;
  z-index: 80;
}
.f-cropper {
  background: var(--surface, #16202a);
  border: 1px solid var(--border, #2c3947);
  border-radius: 10px;
  color: var(--foreground, #e8eef4);
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-width: 92vw;
  padding: 14px;
}
.cropper-head {
  align-items: center;
  display: flex;
  font-size: 13px;
  font-weight: 600;
  justify-content: space-between;
}
.cropper-close {
  background: transparent;
  border: none;
  color: inherit;
  cursor: pointer;
  font-size: 16px;
}
.cropper-stage {
  background: #0b0f14;
  border-radius: 8px;
  overflow: hidden;
  position: relative;
}
.cropper-canvas {
  cursor: grab;
  display: block;
  touch-action: none;
}
.cropper-hint {
  color: #8ea3b5;
  font-size: 12px;
  inset: 0;
  position: absolute;
  text-align: center;
}
.cropper-actions {
  align-items: center;
  display: flex;
  gap: 8px;
}
.cropper-file {
  display: none;
}
.cropper-btn {
  background: var(--surface, #1d2833);
  border: 1px solid var(--border, #2c3947);
  border-radius: 6px;
  color: inherit;
  cursor: pointer;
  font-size: 12px;
  padding: 6px 12px;
}
.cropper-btn.primary:not(:disabled) {
  background: #0b78fe;
  border-color: #0b78fe;
  color: #fff;
}
.cropper-btn:disabled {
  cursor: default;
  opacity: 0.5;
}
.cropper-zoom {
  color: var(--muted-foreground, #8ea3b5);
  display: inline-flex;
  align-items: center;
  font-size: 12px;
  gap: 6px;
}
</style>
