<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  recipeIds,
  recipes,
  streetlightOptions,
  type RecipeId,
  type Streetlights,
} from "./roadSchema";

defineProps<{ selected: RecipeId; streetlights: Streetlights }>();
const emit = defineEmits<{
  select: [id: RecipeId];
  lighting: [placement: Streetlights];
}>();
const { t } = useI18n();

function drag(event: DragEvent, id: RecipeId) {
  event.dataTransfer?.setData("application/x-openscp-road-recipe", id);
}
</script>

<template>
  <aside class="library" :aria-label="t('roadAsset.library')">
    <header>{{ t("roadAsset.library") }}</header>
    <button
      v-for="id in recipeIds"
      :key="id"
      type="button"
      draggable="true"
      :aria-pressed="selected === id"
      @click="emit('select', id)"
      @dragstart="drag($event, id)"
    >
      <strong>{{ t(`roadAsset.recipes.${id}`) }}</strong>
      <span>{{
        t("roadAsset.recipeSummary", {
          count: recipes[id].lanes,
          width: recipes[id].width,
        })
      }}</span>
    </button>
    <fieldset v-if="selected === 'highStreet'" class="lighting">
      <legend>{{ t("roadAsset.streetlights") }}</legend>
      <button
        v-for="option in streetlightOptions"
        :key="option"
        type="button"
        :aria-pressed="streetlights === option"
        @click="emit('lighting', option)"
      >
        {{ t(`roadAsset.lighting.${option}`) }}
      </button>
    </fieldset>
  </aside>
</template>

<style scoped>
.library {
  height: 100%;
  overflow: auto;
  min-inline-size: 0;
  padding: 0 10px 10px;
}
header {
  position: sticky;
  inset-block-start: 0;
  padding-block: 10px;
  background: var(--surface);
  font-size: 12px;
  font-weight: 600;
}
button {
  display: grid;
  gap: 4px;
  width: 100%;
  margin-block-end: 6px;
  padding: 9px;
  text-align: start;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-elevated);
  color: var(--foreground);
  cursor: grab;
}
button[aria-pressed="true"] {
  border-color: var(--primary);
}
button:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
span {
  font-size: 11px;
  color: var(--muted-foreground);
}
.lighting {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 4px;
  border: 0;
  padding: 8px 0 0;
  margin-inline: 0;
  min-inline-size: 0;
  font-size: 12px;
}
.lighting button {
  margin: 0;
  padding: 5px;
  cursor: pointer;
}
</style>
