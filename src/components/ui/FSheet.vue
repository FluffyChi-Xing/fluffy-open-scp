<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from "vue";
import { useReducedMotion, vMotion } from "motion-v";

interface Props {
  label?: string;
  width?: string;
}
const props = defineProps<Props>();
const open = defineModel<boolean>("open", { default: false });

/**
 * 展开/收起动画（motion-v）：面板用 spring 从右侧滑入、遮罩淡入；收起
 * 用更快的 tween，避免关闭拖沓。`prefers-reduced-motion` 下时长归零，
 * 只保留最终状态。
 */
const reduced = useReducedMotion();
const overlayMotion = computed(() => ({
  animate: { opacity: open.value ? 1 : 0 },
  transition: {
    duration: reduced.value ? 0 : open.value ? 0.22 : 0.16,
    ease: [0.2, 0, 0, 1] as [number, number, number, number],
  },
}));
const sheetMotion = computed(() => ({
  animate: { x: open.value ? 0 : "100%" },
  transition: reduced.value
    ? { duration: 0 }
    : open.value
      ? { type: "spring" as const, stiffness: 300, damping: 32, mass: 0.9 }
      : {
          duration: 0.2,
          ease: [0.4, 0, 1, 1] as [number, number, number, number],
        },
}));

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
  <Teleport to="body"
    ><div
      v-motion
      v-bind="overlayMotion"
      class="f-sheet-overlay"
      :class="{ visible: open }"
      :inert="!open"
      :aria-hidden="open ? undefined : 'true'"
      @mousedown.self="open = false"
    >
      <aside
        v-motion
        v-bind="sheetMotion"
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
.f-sheet-overlay {
  background: oklch(0.1 0.01 260 / 0.45);
  -webkit-backdrop-filter: blur(5px);
  backdrop-filter: blur(5px);
  inset: 0;
  opacity: 0;
  pointer-events: none;
  position: fixed;
  /* 主路径是 motion-v 的 spring/tween（内联驱动，WAAPI 优先级更高）；
     这里的 CSS 过渡只在 JS 帧不可用（窗口被挂起）或降级环境下兜底，
     并尊重 reduced-motion。 */
  transition: opacity 220ms cubic-bezier(0.2, 0, 0, 1);
  z-index: 80;
}
@media (prefers-reduced-motion: reduce) {
  .f-sheet-overlay {
    transition: none;
  }
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
  will-change: transform;
  width: min(360px, 92vw);
}
@media (prefers-reduced-motion: reduce) {
  .f-sheet {
    transition: none;
  }
}
.f-sheet.visible {
  transform: translateX(0);
}
</style>
