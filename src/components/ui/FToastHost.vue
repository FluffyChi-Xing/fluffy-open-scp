<script setup lang="ts">
import { onBeforeUnmount, watch } from "vue";
import { useToast } from "@/composables/useToast";

const { toasts, dismiss } = useToast();
const timers = new Map<number, ReturnType<typeof setTimeout>>();
watch(
  toasts,
  (items) => {
    for (const item of items) {
      if (timers.has(item.id)) continue;
      timers.set(
        item.id,
        setTimeout(() => {
          dismiss(item.id);
          timers.delete(item.id);
        }, item.duration),
      );
    }
    for (const [id, timer] of timers)
      if (!items.some((item) => item.id === id)) {
        clearTimeout(timer);
        timers.delete(id);
      }
  },
  { deep: true },
);
onBeforeUnmount(() => {
  for (const timer of timers.values()) clearTimeout(timer);
});
</script>

<template>
  <Teleport to="body"
    ><TransitionGroup
      name="toast"
      tag="ol"
      class="f-toast-host"
      aria-live="polite"
      ><li
        v-for="toast in toasts"
        :key="toast.id"
        class="f-toast"
        :class="`f-toast-${toast.tone}`"
      >
        <div>
          <strong v-if="toast.title">{{ toast.title }}</strong>
          <p>{{ toast.message }}</p>
        </div>
        <button
          type="button"
          :aria-label="$t('toast.dismiss')"
          @click="dismiss(toast.id)"
        >
          ×
        </button>
      </li></TransitionGroup
    ></Teleport
  >
</template>

<style scoped>
.f-toast-host {
  display: grid;
  gap: 10px;
  inset: auto 18px 18px auto;
  list-style: none;
  margin: 0;
  padding: 0;
  position: fixed;
  width: min(380px, calc(100vw - 36px));
  z-index: 120;
}
.f-toast {
  align-items: flex-start;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-inline-start: 3px solid var(--primary);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  padding: 12px 13px;
}
.f-toast-success {
  border-inline-start-color: var(--success);
}
.f-toast-warning {
  border-inline-start-color: var(--warning);
}
.f-toast-error {
  border-inline-start-color: var(--danger);
}
.f-toast strong {
  font-size: 13px;
}
.f-toast p {
  color: var(--muted-foreground);
  font-size: 12px;
  line-height: 1.45;
  margin: 3px 0 0;
}
.f-toast button {
  background: transparent;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  font-size: 19px;
  line-height: 1;
  padding: 0;
}
</style>

<style scoped>
.f-toast {
  --toast-tone: var(--info);
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  backdrop-filter: blur(20px) saturate(130%);
  -webkit-backdrop-filter: blur(20px) saturate(130%);
  border: 1px solid color-mix(in srgb, var(--toast-tone) 42%, var(--border));
  border-radius: var(--radius-lg);
  box-shadow:
    var(--shadow-lg),
    inset 0 1px 0 color-mix(in srgb, var(--foreground) 8%, transparent);
}
.f-toast-success {
  --toast-tone: var(--success);
}
.f-toast-warning {
  --toast-tone: var(--warning);
}
.f-toast-error {
  --toast-tone: var(--danger);
}
.f-toast strong {
  color: var(--toast-tone);
}
.toast-enter-active,
.toast-leave-active,
.toast-move {
  transition:
    opacity 180ms ease-out,
    transform 180ms ease-out;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
.toast-leave-active {
  position: absolute;
  width: 100%;
}
</style>
