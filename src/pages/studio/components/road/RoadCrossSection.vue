<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  crossSection,
  recipeIds,
  type RecipeId,
  type Streetlights,
} from "./roadSchema";

const props = defineProps<{ recipe: RecipeId; streetlights: Streetlights }>();
const emit = defineEmits<{ select: [id: RecipeId] }>();
const { t } = useI18n();
const parts = computed(() => crossSection(props.recipe));

function drop(event: DragEvent) {
  const id = event.dataTransfer?.getData(
    "application/x-openscp-road-recipe",
  ) as RecipeId;
  if (recipeIds.includes(id)) emit("select", id);
}
</script>

<template>
  <section
    class="section"
    :aria-label="t('roadAsset.crossSection')"
    @dragover.prevent
    @drop.prevent="drop"
  >
    <header>
      <strong>{{ t("roadAsset.crossSection") }}</strong>
      <span>{{ t("roadAsset.dropRecipe") }}</span>
    </header>
    <p v-if="recipe === 'highStreet'">
      {{ t("roadAsset.streetlights") }} ·
      {{ t(`roadAsset.lighting.${streetlights}`) }}
    </p>
    <div class="parts">
      <div
        v-for="part in parts"
        :key="part.id"
        class="part"
        :class="part.kind"
        :style="{ flexGrow: part.width }"
      >
        <b v-if="part.direction" aria-hidden="true">{{ part.direction }}</b>
        <span>{{ t(`roadAsset.parts.${part.kind}`) }}</span>
        <small>{{
          t("roadAsset.metres", { width: Number(part.width.toFixed(2)) })
        }}</small>
      </div>
    </div>
  </section>
</template>

<style scoped>
.section {
  height: 100%;
  overflow: auto;
  padding: 12px;
  background: var(--surface);
}
header {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: wrap;
  margin-block-end: 14px;
  font-size: 12px;
}
header span,
small,
p {
  color: var(--muted-foreground);
}
p {
  margin-block: 0 8px;
  font-size: 11px;
}
.parts {
  display: flex;
  gap: 3px;
  min-inline-size: max-content;
}
.part {
  flex-basis: 0;
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 4px;
  min-inline-size: 38px;
  padding: 8px 5px;
  border-block-start: 3px solid var(--border);
  background: var(--surface-elevated);
  font-size: 11px;
}
.part.traffic {
  border-color: var(--primary);
}
.part.walk {
  border-color: #9eb3c6;
}
.part.median {
  border-color: #76a275;
}
.part.rail {
  border-color: #a073c8;
}
b {
  font-size: 16px;
}
</style>
