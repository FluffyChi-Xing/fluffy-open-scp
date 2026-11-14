<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { command } from "@/api/tauri";
import RoadAppearancePreview from "@/pages/packages/components/preview/RoadAppearancePreview.vue";
import type { RoadAppearance } from "@/pages/packages/components/preview/roadAppearanceScene";
import type { RecipeId, Streetlights } from "./roadSchema";

const props = defineProps<{
  project: string;
  gamePackage: string;
  recipe: RecipeId;
  streetlights: Streetlights;
}>();
const { t } = useI18n();
const future = shallowRef(false);
const supportsFuture = computed(() => props.recipe !== "dirt");
watch(supportsFuture, (supported) => {
  if (!supported) future.value = false;
});

const loadRecipe = computed(() => {
  const request = {
    project: props.project,
    gamePackage: props.gamePackage,
    recipe: props.recipe,
    future: supportsFuture.value && future.value,
    streetlights: props.streetlights,
  };
  return () =>
    command<[number, RoadAppearance]>("read_road_recipe_appearance", request);
});
</script>

<template>
  <section class="road-preview" :aria-label="t('roadAsset.preview')">
    <RoadAppearancePreview v-if="gamePackage" :load-recipe="loadRecipe" />
    <p v-else class="source-hint">
      {{ t("roadAsset.appearance.sourceRequired") }}
    </p>
    <div
      v-if="gamePackage && supportsFuture"
      class="appearance-controls"
      role="group"
      :aria-label="t('roadAsset.appearance.style')"
    >
      <button type="button" :aria-pressed="!future" @click="future = false">
        {{ t("roadAsset.appearance.original") }}
      </button>
      <button type="button" :aria-pressed="future" @click="future = true">
        {{ t("roadAsset.appearance.future") }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.road-preview {
  position: relative;
  height: 100%;
  overflow: hidden;
  background: var(--surface-elevated);
}
.appearance-controls {
  position: absolute;
  inset-block-start: 8px;
  inset-inline-end: 10px;
  display: flex;
  gap: 4px;
}
button {
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 4px 8px;
  background: var(--surface);
  color: var(--foreground);
  font-size: 12px;
}
button[aria-pressed="true"] {
  border-color: var(--primary);
}
button:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
.source-hint {
  margin: 12px;
  color: var(--muted-foreground);
  font-size: 12px;
}
</style>
