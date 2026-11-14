<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{ tabId: string; panelId: string }>();
const { t } = useI18n();
const unavailable = ["graffiti", "neon", "interior", "rgba", "hologram", "ground"] as const;
</script>

<template>
  <div class="decal-tabs" role="tablist" :aria-label="t('rasterEditor.sign.tabs')">
    <button :id="tabId" type="button" role="tab" aria-selected="true" :aria-controls="panelId">
      {{ t("rasterEditor.sign.signs") }}
    </button>
    <button v-for="type in unavailable" :key="type" type="button" role="tab" aria-selected="false" disabled>
      {{ t(`rasterEditor.sign.types.${type}`) }}
    </button>
  </div>
</template>

<style scoped>
.decal-tabs {
  display: flex;
  flex: none;
  gap: 4px;
  padding-inline: 16px;
  overflow-x: auto;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}
.decal-tabs button {
  flex: none;
  padding: 10px 12px 8px;
  color: var(--foreground);
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  font: inherit;
  font-size: 12px;
}
.decal-tabs button[aria-selected="true"] {
  border-bottom-color: var(--primary);
  font-weight: 650;
}
.decal-tabs button:disabled { color: var(--muted-foreground); opacity: 0.65; }
.decal-tabs button:focus-visible { outline: 2px solid var(--primary); outline-offset: -3px; }
</style>
