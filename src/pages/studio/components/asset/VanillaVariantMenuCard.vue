<script setup lang="ts">
import { shallowRef, watch } from "vue";
import { isTauri } from "@/api";
import FIcon from "@/components/extensions/FIcon.vue";
import { loadReplicaToolPreview } from "@/lib/game-ui/replica/loader";

const props = defineProps<{
  label: string;
  preview?: string | null;
  selected: boolean;
  disabled: boolean;
}>();
const emit = defineEmits<{ click: [] }>();
const image = shallowRef<string | null>(null);

watch(() => props.preview, async (preview, _, onCleanup) => {
  image.value = null;
  if (!preview) return;
  let current = true;
  onCleanup(() => { current = false; });
  const url = isTauri() ? await loadReplicaToolPreview(preview) : preview;
  if (current) image.value = url;
}, { immediate: true });
</script>

<template>
  <button type="button" class="building-card" :aria-pressed="selected" :disabled="disabled" @click="emit('click')">
    <span class="building-image">
      <img v-if="image" :src="image" alt="" loading="lazy" @error="image = null" />
      <FIcon v-else name="Building2" :size="23" aria-label="" />
    </span>
    <span class="building-name">{{ label }}</span>
    <FIcon :name="selected ? 'CircleCheck' : 'ChevronRight'" :size="16" aria-label="" />
  </button>
</template>

<style scoped>
.building-card { display: grid; grid-template-columns: 54px minmax(0, 1fr) 16px; align-items: center; gap: 10px; min-height: 70px; width: 100%; padding: 7px 10px 7px 7px; border: 1px solid var(--border); border-radius: var(--radius-md); background: var(--surface-elevated); color: var(--muted-foreground); text-align: start; transition: background-color 140ms ease, border-color 140ms ease, scale 100ms ease; }
.building-card:hover:not(:disabled) { background: var(--surface-hover); border-color: var(--border-strong); }
.building-card[aria-pressed="true"] { background: var(--accent); color: var(--accent-foreground); border-color: var(--primary); }
.building-card:focus-visible { outline: 2px solid var(--ring); outline-offset: -2px; }
.building-card:active:not(:disabled) { scale: .96; }
.building-card:disabled { opacity: .6; }
.building-image { display: grid; place-items: center; width: 54px; height: 54px; border-radius: var(--radius-sm); background: var(--surface); overflow: hidden; }
img { width: 100%; height: 100%; object-fit: contain; }
.building-name { font-size: 13px; font-weight: 600; line-height: 1.4; overflow-wrap: anywhere; color: var(--foreground); }
@media (prefers-reduced-motion: reduce) { .building-card { transition: none; } }
</style>
