<script setup lang="ts">
import { computed, shallowRef, useTemplateRef, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import FButton from "@/components/ui/FButton.vue";
import type { CameraCurve } from "./cityCamera";
import type { SampleLimits } from "./cameraCurveEditing";

const props = defineProps<{
  curve: CameraCurve;
  samples: number[] | null;
  unit: string;
  initialIndex: number;
  limits: (index: number) => SampleLimits;
  disabled: boolean;
}>();
const emit = defineEmits<{ select: [index: number] }>();
const draft = defineModel<string>({ required: true });
const canvas = useTemplateRef<HTMLCanvasElement>("chart");
const { t, locale } = useI18n();
const values = computed(() => props.samples ?? props.curve.samples);
const selection = shallowRef(props.initialIndex);
const index = computed(() =>
  Math.max(0, Math.min(selection.value, values.value.length - 1)),
);
const value = computed(() => values.value[index.value]!);
const precise = shallowRef<string | null>(null);
const drag = shallowRef<{
  id: number;
  y: number;
  value: number;
  range: SampleLimits;
  index: number;
  height: number;
}>();
const width = 640;
const height = 212;
const plot = { left: 76, right: 618, top: 20, bottom: 180 };
const formatter = computed(
  () =>
    new Intl.NumberFormat(locale.value, {
      notation: "compact",
      maximumFractionDigits: 2,
    }),
);
const range = computed<SampleLimits>(() => {
  const all = [...props.curve.samples, ...values.value];
  const low = Math.min(...all);
  const high = Math.max(...all);
  const span =
    high - low ||
    Math.max(Math.abs(high) * 0.25, props.unit === "°" ? 12 : 0.01);
  const padding = span * (high === low ? 0.5 : 0.12);
  return [
    props.unit === "m" && low >= 0 ? Math.max(0, low - padding) : low - padding,
    high + padding,
  ];
});

function select(next: number) {
  if (props.disabled) return;
  selection.value = Math.max(0, Math.min(values.value.length - 1, next));
  precise.value = null;
  emit("select", index.value);
}
function edit(next: number) {
  if (props.disabled || !Number.isFinite(next)) return;
  const [min, max] = props.limits(index.value);
  if (min > max) return;
  const sample = Math.fround(Math.max(min, Math.min(max, next)));
  if (!Number.isFinite(sample) || sample === value.value) return;
  const updated = values.value.slice();
  updated[index.value] = sample;
  draft.value = updated.join(" ");
}
function start(event: PointerEvent) {
  if (props.disabled || drag.value || event.button !== 0) return;
  const target = event.currentTarget as HTMLCanvasElement;
  const box = target.getBoundingClientRect();
  if (box.width === 0 || box.height === 0) return;
  const x = ((event.clientX - box.left) / box.width) * width;
  const sample = Math.round(
    ((x - plot.left) / (plot.right - plot.left)) * (values.value.length - 1),
  );
  select(sample);
  target.focus();
  target.setPointerCapture(event.pointerId);
  drag.value = {
    id: event.pointerId,
    y: event.clientY,
    value: value.value,
    range: range.value,
    index: index.value,
    height: (box.height * (plot.bottom - plot.top)) / height,
  };
}
function move(event: PointerEvent) {
  if (drag.value?.id !== event.pointerId || drag.value.index !== index.value)
    return;
  const {
    y,
    value: startValue,
    range: [min, max],
    height: plotHeight,
  } = drag.value;
  const delta = ((y - event.clientY) * (max - min)) / plotHeight;
  edit(startValue + delta * (event.shiftKey ? 0.1 : 1));
}
function stop(event: PointerEvent) {
  if (drag.value?.id === event.pointerId) drag.value = undefined;
}
function keydown(event: KeyboardEvent) {
  if (props.disabled || event.ctrlKey || event.metaKey || event.altKey) return;
  const [min, max] = range.value;
  const step = (max - min) / (event.shiftKey ? 1000 : 100);
  switch (event.key) {
    case "ArrowLeft":
      select(index.value - 1);
      break;
    case "ArrowRight":
      select(index.value + 1);
      break;
    case "Home":
      select(0);
      break;
    case "End":
      select(values.value.length - 1);
      break;
    case "ArrowUp":
      emit("select", index.value);
      edit(value.value + step);
      break;
    case "ArrowDown":
      emit("select", index.value);
      edit(value.value - step);
      break;
    default:
      return;
  }
  event.preventDefault();
}
function commitPrecise() {
  if (precise.value !== null && precise.value.trim() !== "") {
    emit("select", index.value);
    edit(Number(precise.value));
  }
  precise.value = null;
}

watchEffect(
  () => {
    const context = canvas.value?.getContext("2d");
    if (!context) return;
    const [minimum, maximum] = drag.value?.range ?? range.value;
    const x = (i: number, count: number) =>
      plot.left +
      (plot.right - plot.left) * (count === 1 ? 0.5 : i / (count - 1));
    const y = (sample: number) =>
      plot.bottom -
      ((plot.bottom - plot.top) * (sample - minimum)) / (maximum - minimum);
    context.clearRect(0, 0, width, height);
    context.font = "13px sans-serif";
    context.fillStyle = "#899aab";
    context.strokeStyle = "#71859a55";
    context.lineWidth = 1;
    context.textBaseline = "middle";
    context.textAlign = "right";
    for (let row = 0; row <= 4; row++) {
      const tick = minimum + ((maximum - minimum) * row) / 4;
      const location = y(tick);
      context.fillText(formatter.value.format(tick), plot.left - 12, location);
      context.beginPath();
      context.moveTo(plot.left, location);
      context.lineTo(plot.right, location);
      context.stroke();
    }
    context.textBaseline = "top";
    context.textAlign = "left";
    context.fillText(formatter.value.format(1), plot.left, plot.bottom + 10);
    context.textAlign = "right";
    if (values.value.length > 1)
      context.fillText(
        formatter.value.format(values.value.length),
        plot.right,
        plot.bottom + 10,
      );
    const selectedX = x(index.value, values.value.length);
    context.strokeStyle = "#719ed7aa";
    context.beginPath();
    context.moveTo(selectedX, plot.top);
    context.lineTo(selectedX, plot.bottom);
    context.stroke();

    context.save();
    context.beginPath();
    context.rect(
      plot.left - 9,
      plot.top - 9,
      plot.right - plot.left + 18,
      plot.bottom - plot.top + 18,
    );
    context.clip();
    const draw = (samples: number[], color: string, dashed: boolean) => {
      context.beginPath();
      context.strokeStyle = color;
      context.lineWidth = dashed ? 1.5 : 2.6;
      context.setLineDash(dashed ? [6, 5] : []);
      if (samples.length === 1) {
        context.moveTo(plot.left, y(samples[0]!));
        context.lineTo(plot.right, y(samples[0]!));
      } else {
        samples.forEach((sample, i) => {
          if (i === 0) context.moveTo(x(i, samples.length), y(sample));
          else context.lineTo(x(i, samples.length), y(sample));
        });
      }
      context.stroke();
    };
    draw(props.curve.samples, "#899aab", true);
    draw(values.value, "#168af7", false);
    context.fillStyle = "#168af7";
    if (values.value.length <= 64) {
      values.value.forEach((sample, i) => {
        context.beginPath();
        context.arc(x(i, values.value.length), y(sample), 3.4, 0, Math.PI * 2);
        context.fill();
      });
    }
    context.beginPath();
    context.arc(selectedX, y(value.value), 7, 0, Math.PI * 2);
    context.fill();
    context.strokeStyle = "#e1efff";
    context.lineWidth = 2;
    context.stroke();
    context.restore();
  },
  { flush: "post" },
);
</script>

<template>
  <canvas
    ref="chart"
    class="chart"
    :class="{ grabbing: drag }"
    :width="width"
    :height="height"
    :tabindex="disabled ? -1 : 0"
    role="group"
    :aria-label="t('studio.camera.chart')"
    :aria-description="t('studio.camera.chartKeyboard')"
    :aria-disabled="disabled"
    @pointerdown="start"
    @pointermove="move"
    @pointerup="stop"
    @pointercancel="stop"
    @lostpointercapture="stop"
    @keydown="keydown"
  />
  <div class="chart-footer">
    <span>{{ t("studio.camera.sampleCount", { count: values.length }) }}</span>
    <span class="legend"
      ><i />{{ t("studio.camera.source") }} <i />{{
        t("studio.camera.draft")
      }}</span
    >
  </div>
  <div class="sample-controls">
    <div class="sample-nav">
      <FButton
        variant="secondary"
        :disabled="disabled || index === 0"
        :aria-label="t('studio.camera.previousSample')"
        @click="select(index - 1)"
        >‹</FButton
      >
      <span>{{
        t("studio.camera.selectedSample", {
          index: index + 1,
          count: values.length,
        })
      }}</span>
      <FButton
        variant="secondary"
        :disabled="disabled || index === values.length - 1"
        :aria-label="t('studio.camera.nextSample')"
        @click="select(index + 1)"
        >›</FButton
      >
    </div>
    <label class="sample-value">
      <span>{{ t("studio.camera.sampleValue", { unit }) }}</span>
      <input
        type="number"
        step="any"
        :disabled="disabled"
        :value="precise ?? Number(value.toPrecision(8))"
        @input="precise = ($event.target as HTMLInputElement).value"
        @change="commitPrecise"
        @keydown.enter.prevent="commitPrecise"
      />
    </label>
  </div>
  <p class="hint">{{ t("studio.camera.chartHint") }}</p>
  <details class="batch-edit">
    <summary>{{ t("studio.camera.batchSamples") }}</summary>
    <label class="sample-label">
      {{ t("studio.camera.samples", { unit }) }}
      <textarea
        v-model="draft"
        rows="4"
        spellcheck="false"
        :disabled="disabled"
        :aria-invalid="!samples"
      />
    </label>
    <p class="hint">{{ t("studio.camera.sampleHint") }}</p>
  </details>
  <p v-if="!samples" class="error" role="status">
    {{ t("studio.camera.invalidSamples") }}
  </p>
</template>

<style scoped>
.chart {
  display: block;
  width: 100%;
  height: auto;
  background: var(--background);
  border-radius: var(--radius-sm);
  cursor: ns-resize;
  touch-action: none;
}
.chart.grabbing {
  cursor: grabbing;
}
.chart:focus-visible {
  outline: 2px solid var(--brand);
  outline-offset: 2px;
}
.chart[aria-disabled="true"] {
  cursor: default;
}
.chart-footer,
.legend,
.sample-controls,
.sample-nav {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.chart-footer,
.hint,
.error,
.sample-nav {
  font-size: 12px;
  color: var(--muted-foreground);
}
.legend i {
  width: 12px;
  border-top: 2px dashed #899aab;
}
.legend i:last-of-type {
  border-top: 2px solid #168af7;
}
.sample-controls {
  align-items: end;
  gap: 12px;
}
.sample-nav {
  gap: 9px;
  min-height: 38px;
}
.sample-nav button {
  min-height: 36px;
  min-width: 36px;
  padding: 0;
  font-size: 18px;
}
.sample-value,
.sample-label {
  display: grid;
  gap: 6px;
  font-size: 13px;
}
.sample-value {
  flex: 1 1 108px;
  max-width: 180px;
}
.sample-value input,
.sample-label textarea {
  min-width: 0;
  width: 100%;
  padding: 8px 10px;
  color: var(--foreground);
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  font: 13px/1.5 var(--font-mono, monospace);
}
.sample-label textarea {
  resize: vertical;
}
.batch-edit summary {
  cursor: pointer;
  font-size: 13px;
}
.batch-edit[open] summary,
.batch-edit .sample-label {
  margin-bottom: 8px;
}
.hint,
.error {
  margin: 0;
}
.error {
  color: var(--destructive);
}
</style>
