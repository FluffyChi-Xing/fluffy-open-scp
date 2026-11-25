<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{ tabId: string; panelId: string }>();
const { t } = useI18n();
const unavailable = ["graffiti", "neon", "interior", "rgba", "hologram", "ground"] as const;
</script>

<template>
  <div class="decal-type-field">
    <span :id="`${tabId}-label`">{{ t("rasterEditor.sign.tabs") }}</span>
    <div class="decal-tabs" role="tablist" :aria-labelledby="`${tabId}-label`">
      <button :id="tabId" class="sign-control" type="button" role="tab" aria-selected="true" :aria-controls="panelId">
        {{ t("rasterEditor.sign.signs") }}
      </button>
      <button v-for="type in unavailable" :key="type" class="sign-control" type="button" role="tab" aria-selected="false" disabled>
        {{ t(`rasterEditor.sign.types.${type}`) }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.decal-type-field {
  display: grid;
  gap: var(--sign-row-gap);
  font-size: 12px;
  font-weight: 600;
}
.decal-tabs {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--sign-row-gap);
}
.decal-tabs button {
  text-align: start;
}
.decal-tabs button[aria-selected="true"] {
  grid-column: 1 / -1;
  border-color: var(--primary);
  background: var(--accent);
  font-weight: 650;
}
.decal-tabs button:disabled { color: var(--muted-foreground); opacity: 0.65; }
</style>
