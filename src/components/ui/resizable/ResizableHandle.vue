<script setup lang="ts">
import { SplitterResizeHandle } from "reka-ui";

interface Props {
  orientation: "horizontal" | "vertical";
  label: string;
}

defineProps<Props>();
</script>

<template>
  <SplitterResizeHandle
    class="resizable-handle"
    :class="`is-${orientation}`"
    :aria-label="label"
    :hit-area-margins="{ coarse: 12, fine: 8 }"
  >
    <span aria-hidden="true" />
  </SplitterResizeHandle>
</template>

<style scoped>
.resizable-handle {
  align-items: center;
  display: flex;
  flex: none;
  justify-content: center;
  outline: none;
  position: relative;
  z-index: 2;
}
.resizable-handle::before {
  background: var(--border);
  content: "";
  inset: 0;
  position: absolute;
  transition: background 120ms ease;
}
.resizable-handle span {
  background: var(--subtle-foreground);
  border-radius: 999px;
  opacity: 0;
  position: relative;
  transition: opacity 120ms ease;
}
.resizable-handle.is-horizontal {
  cursor: col-resize;
  width: 7px;
}
.resizable-handle.is-horizontal::before {
  inset-inline: 3px;
}
.resizable-handle.is-horizontal span {
  height: 30px;
  width: 2px;
}
.resizable-handle.is-vertical {
  cursor: row-resize;
  height: 7px;
}
.resizable-handle.is-vertical::before {
  inset-block: 3px;
}
.resizable-handle.is-vertical span {
  height: 2px;
  width: 30px;
}
.resizable-handle:hover::before,
.resizable-handle:focus-visible::before,
.resizable-handle[data-state="drag"]::before {
  background: var(--primary);
}
.resizable-handle:hover span,
.resizable-handle:focus-visible span,
.resizable-handle[data-state="drag"] span {
  opacity: 1;
}
.resizable-handle:focus-visible {
  box-shadow: inset 0 0 0 2px var(--ring);
}
</style>
