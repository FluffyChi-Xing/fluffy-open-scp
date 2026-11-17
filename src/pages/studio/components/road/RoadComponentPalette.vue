<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { RecipeId } from "./roadSchema";
import {
  availableComponents,
  roadComponentDragType,
  type RoadComponent,
  type RoadComposition,
} from "./roadComposition";

const props = defineProps<{
  recipe: RecipeId;
  composition?: RoadComposition;
}>();
const emit = defineEmits<{ toggle: [component: RoadComponent] }>();
const { t } = useI18n();
const components = computed(() => availableComponents(props.recipe));

function drag(event: DragEvent, component: RoadComponent) {
  event.dataTransfer?.setData(roadComponentDragType, component);
}
</script>

<template>
  <fieldset v-if="components.length" class="components">
    <legend>{{ t("roadAsset.components.title") }}</legend>
    <button
      v-for="component in components"
      :key="component"
      type="button"
      draggable="true"
      :aria-pressed="composition?.[component] ?? false"
      @click="emit('toggle', component)"
      @dragstart="drag($event, component)"
    >
      {{ t(`roadAsset.components.${component}`) }}
    </button>
  </fieldset>
</template>

<style scoped>
.components {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin: 8px 10px;
  padding: 0;
  border: 0;
  font-size: 12px;
}
legend {
  padding-block-end: 4px;
}
button {
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface-elevated);
  color: var(--foreground);
  padding: 4px 8px;
  cursor: grab;
}
button[aria-pressed="true"] {
  border-color: var(--primary);
}
</style>
