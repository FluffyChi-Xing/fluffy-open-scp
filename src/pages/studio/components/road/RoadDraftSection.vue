<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import {
  insertStrip,
  isStripKind,
  isValidStripWidth,
  moveStrip,
  moveStripDragType,
  stripDragType,
  type RoadStrip,
  type StripKind,
} from "./roadDraft";

const props = defineProps<{
  strips: RoadStrip[];
  widths?: Record<StripKind, number>;
}>();
const emit = defineEmits<{
  update: [strips: RoadStrip[]];
}>();
const { t } = useI18n();
const activeGap = shallowRef<number>();
const total = computed(() =>
  Number(props.strips.reduce((sum, strip) => sum + strip.width, 0).toFixed(2)),
);

function dragOver(event: DragEvent, gap: number) {
  const transfer = event.dataTransfer;
  if (!transfer) return;
  const moving = transfer.types.includes(moveStripDragType);
  if (!moving && !transfer.types.includes(stripDragType)) return;
  event.preventDefault();
  transfer.dropEffect = moving ? "move" : "copy";
  activeGap.value = gap;
}

function drop(event: DragEvent, gap: number) {
  activeGap.value = undefined;
  const transfer = event.dataTransfer;
  if (!transfer) return;
  const id = transfer.getData(moveStripDragType);
  const kind = transfer.getData(stripDragType);
  if (id) emit("update", moveStrip(props.strips, id, gap));
  else if (isStripKind(kind))
    emit("update", insertStrip(props.strips, gap, kind, props.widths?.[kind]));
}

function drag(event: DragEvent, id: string) {
  event.dataTransfer?.setData(moveStripDragType, id);
}

function nearestGap(event: DragEvent, index: number) {
  const frame = (event.currentTarget as HTMLElement).getBoundingClientRect();
  return event.clientX < frame.left + frame.width / 2 ? index : index + 1;
}

function setWidth(event: Event, strip: RoadStrip) {
  const input = event.currentTarget as HTMLInputElement;
  const width = input.valueAsNumber;
  if (!isValidStripWidth(width)) {
    input.value = String(strip.width);
    return;
  }
  emit(
    "update",
    props.strips.map((part) =>
      part.id === strip.id ? { ...part, width } : part,
    ),
  );
}
</script>

<template>
  <section class="section" :aria-label="t('roadAsset.crossSection')">
    <header>
      <strong>{{ t("roadAsset.crossSection") }}</strong>
      <span>{{ t("roadAsset.draft.totalWidth", { width: total }) }}</span>
    </header>
    <div
      v-if="!strips.length"
      class="empty"
      :class="{ active: activeGap === 0 }"
      @dragover="dragOver($event, 0)"
      @dragleave.self="activeGap = undefined"
      @drop.prevent="drop($event, 0)"
    >
      {{ t("roadAsset.draft.empty") }}
    </div>
    <div v-else class="strips" @dragend="activeGap = undefined">
      <template v-for="(strip, index) in strips" :key="strip.id">
        <div
          class="gap"
          :class="{ active: activeGap === index }"
          @dragover="dragOver($event, index)"
          @dragleave="activeGap = undefined"
          @drop.prevent="drop($event, index)"
        />
        <article
          class="strip"
          :class="strip.kind"
          @dragover="dragOver($event, nearestGap($event, index))"
          @dragleave.self="activeGap = undefined"
          @drop.prevent="drop($event, nearestGap($event, index))"
        >
          <strong draggable="true" @dragstart="drag($event, strip.id)">
            {{ t(`roadAsset.strips.${strip.kind}`) }}
          </strong>
          <label>
            <input
              type="number"
              step="any"
              :value="strip.width"
              :aria-label="
                t('roadAsset.draft.stripWidth', {
                  strip: t(`roadAsset.strips.${strip.kind}`),
                })
              "
              @change="setWidth($event, strip)"
            />
            <span>{{ t("roadAsset.draft.metreUnit") }}</span>
          </label>
          <div class="actions">
            <button
              type="button"
              :disabled="index === 0"
              :aria-label="t('roadAsset.draft.moveLeft')"
              @click="emit('update', moveStrip(strips, strip.id, index - 1))"
            >
              ←
            </button>
            <button
              type="button"
              :disabled="index === strips.length - 1"
              :aria-label="t('roadAsset.draft.moveRight')"
              @click="emit('update', moveStrip(strips, strip.id, index + 2))"
            >
              →
            </button>
            <button
              type="button"
              :aria-label="t('roadAsset.draft.remove')"
              @click="
                emit(
                  'update',
                  strips.filter((part) => part.id !== strip.id),
                )
              "
            >
              ×
            </button>
          </div>
        </article>
      </template>
      <div
        class="gap"
        :class="{ active: activeGap === strips.length }"
        @dragover="dragOver($event, strips.length)"
        @dragleave="activeGap = undefined"
        @drop.prevent="drop($event, strips.length)"
      />
    </div>
  </section>
</template>

<style scoped>
.section {
  height: 100%;
  overflow: auto;
  padding: 10px;
  font-size: 12px;
}
header,
.actions,
label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
}
header {
  margin-block-end: 8px;
}
header span,
label span,
.empty {
  color: var(--muted-foreground);
}
.empty {
  display: grid;
  place-items: center;
  min-block-size: 72px;
  border: 1px dashed var(--border);
  border-radius: 4px;
  text-align: center;
}
.strips {
  display: flex;
  min-inline-size: max-content;
}
.strip {
  display: grid;
  gap: 4px;
  padding: 5px;
  inline-size: 96px;
  border-block-start: 2px solid var(--border);
  background: var(--surface-elevated);
}
.strip strong {
  cursor: grab;
  font-size: 11px;
}
.forward,
.backward {
  border-color: var(--primary);
}
.median,
.trees {
  border-color: #7a9e68;
}
.rail {
  border-color: #a78ccc;
}
.gap {
  flex: 0 0 14px;
  border: 1px solid transparent;
}
.gap.active,
.empty.active {
  border-color: var(--primary);
  background: var(--surface-elevated);
}
input,
button {
  background: var(--surface);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: 3px;
}
input {
  inline-size: 60px;
  padding-inline: 3px;
}
button {
  flex: 1;
}
button:disabled {
  opacity: 0.45;
}
</style>
