<script setup lang="ts">
import { computed, shallowRef, useId, useTemplateRef, watch } from "vue";
import FSelect from "@/components/ui/FSelect.vue";
import {
  animationLabel,
  readAnimation,
  type AnimationBinding,
  type AnimationSource,
} from "./animationPreview";

const props = defineProps<{ source: AnimationSource; clips: AnimationBinding[]; refined: boolean }>();
const host = useTemplateRef<HTMLDivElement>("host");
const selectId = useId();
const selected = shallowRef(String(props.clips[0]?.id ?? ""));
const options = computed(() => props.clips.map((clip) => ({
  value: String(clip.id),
  label: animationLabel(clip.id),
})));
const binding = computed(() => props.clips.find((clip) => String(clip.id) === selected.value));
const ratio = shallowRef(0);
const ready = shallowRef(false);
const failed = shallowRef(false);
const playing = shallowRef(false);
const sample = shallowRef<((ratio: number) => void) | null>(null);

watch(binding, () => { ratio.value = 0; });
watch([host, binding, () => props.refined], ([element, clip, refined], _, onCleanup) => {
  ready.value = false;
  failed.value = false;
  playing.value = false;
  sample.value = null;
  if (!element || !clip) return;
  let disposed = false;
  let release = () => {};
  onCleanup(() => { disposed = true; release(); });
  void (async () => {
    const [{ createAnimationScene }, payload] = await Promise.all([
      import("./animationScene"),
      readAnimation(props.source, clip.id, refined),
    ]);
    if (disposed) return;
    const scene = await createAnimationScene(element, payload, clip, () => disposed);
    if (!scene) return;
    if (disposed) { scene.dispose(); return; }
    release = scene.dispose;
    sample.value = scene.sample;
    scene.sample(ratio.value);
    ready.value = true;
  })().catch(() => { if (!disposed) failed.value = true; });
});

watch(ratio, (value) => sample.value?.(value), { flush: "sync" });
watch([playing, ready], ([active, loaded], _, onCleanup) => {
  if (!active || !loaded) return;
  if (ratio.value >= 1) ratio.value = 0;
  const clip = binding.value;
  const durationMs = clip?.resourceId === null ? Math.max(1, clip.end * 1000) : 2000;
  let previous: number | undefined;
  let frame = requestAnimationFrame(function advance(now) {
    if (previous !== undefined) ratio.value = Math.min(1, ratio.value + (now - previous) / durationMs);
    previous = now;
    if (ratio.value >= 1) playing.value = false;
    else frame = requestAnimationFrame(advance);
  });
  onCleanup(() => cancelAnimationFrame(frame));
});
</script>

<template>
  <section class="animation-stage" :aria-label="$t('package.animationPreview')">
    <div class="animation-controls">
      <div class="animation-clip">
        <label :for="selectId">{{ $t("package.animationClip") }}</label>
        <FSelect :id="selectId" v-model="selected" :options="options" />
      </div>
      <label v-if="binding">
        <span>{{ binding.resourceId === null
          ? $t("package.animationProgress")
          : $t("package.animationResourceLevel", { id: animationLabel(binding.resourceId) }) }}</span>
        <input v-model.number="ratio" type="range" min="0" max="1" step="0.01" :disabled="!ready" @pointerdown="playing = false" @keydown="playing = false" />
        {{ Math.round(ratio * 100) }}%
      </label>
      <button type="button" :disabled="!ready" @click="playing = !playing">
        {{ $t(playing ? "package.animationPause" : "package.animationPlay") }}
      </button>
    </div>
    <div ref="host" class="animation-canvas" />
    <p v-if="failed" class="animation-message" role="alert">{{ $t("package.animationLoadFailed") }}</p>
    <p v-else-if="!ready" class="animation-message" role="status">{{ $t("common.loading") }}</p>
  </section>
</template>

<style scoped>
.animation-stage {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  background: var(--background);
  color: var(--foreground);
}
.animation-controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 16px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
}
.animation-controls label,
.animation-clip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
}
.animation-clip :deep(.f-select-anchor) { min-width: 150px; }
.animation-controls button {
  background: var(--background);
  color: inherit;
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 4px 8px;
}
.animation-canvas { min-width: 0; min-height: 0; position: relative; }
.animation-message { position: absolute; inset: 50% 0 auto; text-align: center; pointer-events: none; }
</style>
