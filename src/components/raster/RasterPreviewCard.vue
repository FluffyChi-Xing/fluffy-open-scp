<script setup lang="ts">
import { ref, nextTick, onBeforeUnmount } from "vue";
import { useI18n } from "vue-i18n";
import { Maximize2, RefreshCw, X } from "lucide-vue-next";
import FSheet from "@/components/ui/FSheet.vue";
defineProps<{ title: string; refreshable?: boolean }>();
const emit = defineEmits<{ refresh: [] }>();
const { t } = useI18n();
const open = ref(false);
const trigger = ref<HTMLButtonElement>();
const closeButton = ref<HTMLButtonElement>();
function closeOnEscape(event: KeyboardEvent) {
  if (open.value && event.key === "Escape") {
    event.stopImmediatePropagation();
    event.preventDefault();
    void setOpen(false);
  }
}
window.addEventListener("keydown", closeOnEscape, true);
onBeforeUnmount(() =>
  window.removeEventListener("keydown", closeOnEscape, true),
);
function trapFocus(event: KeyboardEvent) {
  const root = event.currentTarget as HTMLElement;
  const items = [
    ...root.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), [tabindex="0"]',
    ),
  ].filter((el) => el.getClientRects().length);
  const first = items[0],
    last = items.at(-1);
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last?.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first?.focus();
  }
}
async function setOpen(value: boolean) {
  open.value = value;
  await nextTick();
  (value ? closeButton.value : trigger.value)?.focus();
}
</script>
<template>
  <section class="preview-card">
    <header>
      <h3>{{ title }}</h3>
      <div class="header-actions">
        <button v-if="refreshable" :title="t('rasterEditor.refreshPreview')" :aria-label="t('rasterEditor.refreshPreview')" @click="emit('refresh')">
          <RefreshCw :size="14" />
        </button>
        <button
          ref="trigger"
          :title="t('rasterEditor.expand')"
          :aria-label="t('rasterEditor.expand')"
          @click="setOpen(true)"
        >
          <Maximize2 :size="14" />
        </button>
      </div>
    </header>
    <div v-if="!open" class="preview-content"><slot /></div>
    <FSheet :open="open" :label="title" width="100vw" @update:open="setOpen">
      <div v-if="open" class="fullscreen" @keydown.tab="trapFocus">
        <header>
          <h3>{{ title }}</h3>
          <div class="header-actions">
            <button v-if="refreshable" :title="t('rasterEditor.refreshPreview')" :aria-label="t('rasterEditor.refreshPreview')" @click="emit('refresh')">
              <RefreshCw :size="18" />
            </button>
            <button
              ref="closeButton"
              :aria-label="t('rasterEditor.close')"
              @click="setOpen(false)"
            >
              <X :size="20" />
            </button>
          </div>
        </header>
        <div class="preview-content"><slot /></div>
      </div>
    </FSheet>
  </section>
</template>
<style scoped>
.preview-card {
  margin-block-end: 18px;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-block-end: 8px;
}
h3 {
  font-size: 12px;
  margin: 0;
}
.header-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}
button {
  display: grid;
  place-items: center;
  padding: 6px;
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--foreground);
  background: var(--surface);
  cursor: pointer;
}
button:focus-visible {
  outline: 2px solid var(--primary);
}
.preview-content {
  height: 180px;
  min-height: 0;
}
.fullscreen {
  padding: 20px;
  height: 100%;
  display: flex;
  flex-direction: column;
}
.fullscreen .preview-content {
  flex: 1;
}
</style>
