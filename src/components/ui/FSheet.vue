<script setup lang="ts">
import { onBeforeUnmount, watch } from "vue";

interface Props {
  label?: string;
  width?: string;
  /** Render the panel in the current layout instead of teleporting to an overlay. */
  embedded?: boolean;
}
const props = defineProps<Props>();
const open = defineModel<boolean>("open", { default: false });

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") open.value = false;
}
window.addEventListener("keydown", onKeydown);
watch(open, (value) => {
  document.body.style.overflow = value ? "hidden" : "";
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  document.body.style.overflow = "";
});
</script>

<template>
  <div v-if="props.embedded" class="f-sheet-embedded"><slot /></div>
  <Teleport v-else to="body"
    ><div
      class="f-sheet-overlay"
      :class="{ visible: open }"
      :inert="!open"
      :aria-hidden="open ? undefined : 'true'"
      @mousedown.self="open = false"
    >
      <aside
        class="f-sheet"
        :class="{ visible: open }"
        :style="props.width ? { width: props.width } : undefined"
        role="dialog"
        aria-modal="true"
        :aria-label="props.label"
      >
        <slot />
      </aside></div
  ></Teleport>
</template>

<style scoped>
.f-sheet {
  overscroll-behavior: contain;
}
.f-sheet-embedded {
  display: flex;
  min-height: 0;
  width: 100%;
}
.f-sheet-overlay {
  background: oklch(0.1 0.01 260 / 0.45);
  -webkit-backdrop-filter: blur(5px);
  backdrop-filter: blur(5px);
  inset: 0;
  opacity: 0;
  pointer-events: none;
  position: fixed;
  transition: opacity 220ms cubic-bezier(0.2, 0, 0, 1);
  z-index: 80;
}
.f-sheet-overlay.visible {
  opacity: 1;
  pointer-events: auto;
}
.f-sheet {
  background: color-mix(in srgb, var(--surface) 84%, transparent);
  -webkit-backdrop-filter: blur(24px) saturate(140%);
  backdrop-filter: blur(24px) saturate(140%);
  border-left: 1px solid
    color-mix(in srgb, var(--foreground) 12%, var(--border));
  box-shadow: var(--shadow-xl);
  display: flex;
  flex-direction: column;
  height: 100%;
  inset: 0 0 0 auto;
  overflow-y: auto;
  position: absolute;
  transform: translateX(100%);
  transition: transform 280ms cubic-bezier(0.2, 0, 0, 1);
  width: min(360px, 92vw);
}
.f-sheet.visible {
  transform: translateX(0);
}
</style>
