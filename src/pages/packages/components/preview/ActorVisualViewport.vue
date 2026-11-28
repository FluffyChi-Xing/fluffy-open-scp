<script setup lang="ts">
import { computed, shallowRef, useId, useTemplateRef, watch } from "vue";
import FSelect from "@/components/ui/FSelect.vue";
import type { ActorVisualPayload } from "./actorVisualAssets";
import type { ActorLook } from "./actorLook";
const props = defineProps<{
  payload: ActorVisualPayload;
  look: ActorLook;
  scale: number;
}>();
const host = useTemplateRef<HTMLElement>("host");
const failed = shallowRef(false),
  ready = shallowRef(false),
  count = shallowRef(0),
  selected = shallowRef("0");
const id = useId();
const setClip = shallowRef<((index: number) => void) | null>(null);
const options = computed(() =>
  Array.from({ length: count.value }, (_, i) => ({
    value: String(i),
    label: String(i + 1),
  })),
);
watch(selected, (value) => setClip.value?.(Number(value)));
watch(
  [host, () => props.payload, () => props.look, () => props.scale],
  ([element], _, onCleanup) => {
    failed.value = false;
    ready.value = false;
    setClip.value = null;
    if (!element) return;
    let cancelled = false,
      release = () => {};
    onCleanup(() => {
      cancelled = true;
      release();
    });
    void (async () => {
      const { createActorVisualScene } = await import("./actorVisualScene");
      const scene = await createActorVisualScene(
        element,
        props.payload,
        props.look,
        props.scale,
        () => cancelled,
      );
      if (cancelled) {
        scene.dispose();
        return;
      }
      release = scene.dispose;
      count.value = scene.clipCount;
      selected.value = "0";
      setClip.value = scene.setClip;
      ready.value = true;
    })().catch(() => {
      if (!cancelled) failed.value = true;
    });
  },
);
</script>
<template>
  <section class="actor-visual-stage">
    <div v-if="count > 1" class="clip-picker">
      <label :for="id">{{ $t("package.scene.actorClip") }}</label
      ><FSelect :id="id" v-model="selected" :options="options" />
    </div>
    <div ref="host" class="actor-visual-canvas" />
    <p v-if="failed" role="status">{{ $t("package.scene.visualFailed") }}</p>
    <p v-else-if="!ready" role="status">
      {{ $t("package.scene.modelLoading") }}
    </p>
    <p v-else-if="!count" role="status">
      {{ $t("package.scene.noNativeAnimation") }}
    </p>
    <p v-else>{{ $t("package.scene.loopingNative", { n: count }) }}</p>
  </section>
</template>
<style scoped>
.actor-visual-stage {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 8px;
  min-width: 0;
}
.actor-visual-canvas {
  min-width: 0;
  aspect-ratio: 16 / 9;
  min-height: 180px;
  max-height: 320px;
  width: 100%;
  overflow: hidden;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
.actor-visual-canvas :deep(canvas) {
  display: block;
  max-width: 100%;
  max-height: 100%;
}
.clip-picker {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 12px;
}
p {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 12px;
  line-height: 1.6;
}
</style>
