<script setup lang="ts">
import { computed, defineAsyncComponent, shallowRef, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowRight, Eraser, FolderOpen, Map, MousePointer2, Paintbrush, Save, Undo2 } from "lucide-vue-next";
import { useGamePackagesStore } from "@/stores/gamePackages";
import FButton from "@/components/ui/FButton.vue";
import FSelect from "@/components/ui/FSelect.vue";
import FSlider from "@/components/ui/FSlider.vue";
import { ecoChannel } from "@/components/map/map-fields";
import { hasStateResource } from "@/components/map/map-state-fields";
import { windArrowRotation } from "@/lib/region-map";
import MapResourceOverview from "./MapResourceOverview.vue";
import { layers, useMapResources, windDirectionId } from "./useMapResources";

const MapPreviewPane = defineAsyncComponent(() => import("@/components/map/MapPreviewPane.vue"));
const props = defineProps<{ project: string; overview: boolean }>();
const emit = defineEmits<{ back: []; saved: [] }>();
const { t, locale, n } = useI18n();
const fieldId = useId();
const packages = useGamePackagesStore();
const { packagePath, saveDirectory, group, regions, data, schema, selected, busy, error, dirty, choosing,
  activeCity, renderResources, abundance, list, chooseFile, chooseSave, openRegion, stamp, placeWind, undo, save } = useMapResources(props.project);
const radius = shallowRef(48);
const value = shallowRef(32768);
const windAngle = shallowRef(0);
const mode = shallowRef<"select" | "paint" | "erase">("select");
const activeResource = shallowRef<string | null>(null);
const layerId = shallowRef<number>(layers[0].id);
const modes = [{ key: "select", icon: MousePointer2 }, { key: "paint", icon: Paintbrush }, { key: "erase", icon: Eraser }] as const;
const terrainPackages = computed(() => packages.opened.map((entry) => entry.package.path)
  .filter((path) => /(?:^|[/\\])(?:SimCity_)?RegionTerrain[^/\\]*\.package$/i.test(path))
  .map((path) => ({ value: path, label: path.split(/[/\\]/).at(-1) ?? path })));
const chinese = computed(() => locale.value.startsWith("zh"));
const title = computed(() => (chinese.value ? data.value?.displayName : data.value?.displayNameEn)
  ?? data.value?.displayName ?? data.value?.displayNameEn ?? schema.value?.source.group);
const regionOptions = computed(() => regions.value.map((region) => ({
  value: region.group,
  label: (chinese.value ? region.displayName : region.displayNameEn) ?? region.displayName ?? region.displayNameEn ?? region.group,
})));
const cityPlots = computed(() => data.value?.plots.filter((plot) => plot.kind === "city") ?? []);
const cityOptions = computed(() => cityPlots.value.map((plot) => ({
  value: plot.uid,
  label: (chinese.value ? plot.name : plot.nameEn) ?? plot.name ?? plot.nameEn ?? plot.uid,
})));
const activePlot = computed(() => cityPlots.value.find((plot) => plot.uid === selected.value));
const available = computed(() => layers.filter((layer) => layer.key === "wind"
  ? activeCity.value?.windDirection != null : activeCity.value?.maps.some((map) => map.id === layer.id)));
const resource = computed(() => available.value.find((layer) => layer.id === layerId.value));
const eraseOnly = computed(() => resource.value?.key === "groundPollution");
const isWind = computed(() => resource.value?.key === "wind");
const currentWind = computed(() => renderResources.value?.windArrows?.find((arrow) => arrow.site === selected.value)?.direction);
const layerOptions = computed(() => available.value.map((layer) => ({
  value: String(layer.id), label: t(`mapResources.${layer.key === "wind" ? "windDirection" : layer.key}`),
})));
const previewOptions = computed(() => [
  ...["soil", "waterTable", "cityWaterTable", "forest", "coal", "ore", "oil"].map((kind) => ({
    kind,
    label: t(`studio.map.res${kind[0]!.toUpperCase() + kind.slice(1)}`),
    available: ecoChannel(kind) !== null || hasStateResource(renderResources.value ?? null, kind),
  })),
  ...((renderResources.value?.windArrows?.length ?? 0) > 0
    ? [{ kind: "wind", label: t("mapResources.windDirection"), available: true }] : []),
  ...layers.filter((layer) => ["radiation", "groundPollution"].includes(layer.key)
    && hasStateResource(renderResources.value ?? null, layer.key)).map(({ key }) => ({
    kind: key, label: t(`mapResources.${key}`), available: true,
  })),
]);
const resourceEditor = computed(() => ({
  data: renderResources.value,
  editing: !busy.value && !!resource.value && mode.value !== "select",
  radius: isWind.value ? 0 : radius.value,
  pointPlacement: isWind.value,
}));
const visibleModes = computed(() => isWind.value ? modes.filter((tool) => tool.key !== "erase") : modes);
watch([selected, currentWind], ([, angle]) => { if (angle !== undefined) windAngle.value = angle; });
watch(isWind, (enabled) => {
  if (!enabled) return;
  activeResource.value = "wind";
  if (mode.value === "erase") mode.value = "select";
});
watch([activeResource, activeCity], ([layer, city]) => {
  if (layer === "wind" && city?.windDirection != null) layerId.value = windDirectionId;
});
watch(eraseOnly, (only) => { if (only && mode.value === "paint") mode.value = "erase"; });
watch(available, (options) => {
  if (!options.some((option) => option.id === layerId.value)) layerId.value = options[0]?.id ?? layers[0].id;
});
watch(data, () => {
  activeResource.value = null;
  mode.value = "select";
});
watch([mode, resource], ([current, layer]) => {
  if (current !== "select" && layer) activeResource.value = layer.key;
});

function selectCity(uid: string) {
  if (!cityPlots.value.some((plot) => plot.uid === uid)) return;
  selected.value = uid;
  mode.value = "select";
}
function editCity(uid: string) {
  selectCity(uid);
  emit("back");
}
function apply(points: [number, number][]) {
  if (mode.value === "select" || !resource.value) return;
  if (isWind.value) {
    if (points[0]) placeWind(points[0], windAngle.value);
    return;
  }
  stamp(resource.value.id, points, radius.value, mode.value === "erase" ? 0 : value.value);
}
async function commit() {
  const result = await save();
  if (result) emit("saved");
  return result;
}
defineExpose({ save: commit });
</script>

<template>
  <div class="workbench">
    <MapResourceOverview
v-if="overview" :data="data" :group="schema?.source.group ?? ''" :abundance="abundance"
      @back="emit('back')" @select="editCity" />
    <div v-else-if="!data || choosing" class="setup">
      <section class="source-panel">
        <Map :size="26" class="source-icon" aria-hidden="true" />
        <h2>{{ t("mapResources.source") }}</h2>
        <p>{{ t(choosing ? "mapResources.replace" : saveDirectory ? "mapResources.saveHint" : "mapResources.sourceHint") }}</p>
        <div v-if="terrainPackages.length" class="field">
          <label :for="`${fieldId}-package`">{{ t("mapResources.loaded") }}</label>
          <FSelect
:id="`${fieldId}-package`" :model-value="terrainPackages.some(({ value }) => value === packagePath) ? packagePath : ''"
            :options="terrainPackages" :placeholder="t('mapResources.selectPackage')" :disabled="busy" @update:model-value="list" />
        </div>
        <div class="source-buttons">
          <FButton variant="secondary" :disabled="busy" @click="chooseFile">
            <FolderOpen :size="16" aria-hidden="true" /> {{ t("mapResources.choose") }}
          </FButton>
          <FButton variant="secondary" :disabled="busy" @click="chooseSave">
            <Save :size="16" aria-hidden="true" /> {{ t("mapResources.chooseSave") }}
          </FButton>
        </div>
        <div v-if="saveDirectory" class="source-buttons">
          <p class="filename" :title="saveDirectory">{{ t("mapResources.saveSource") }} {{ saveDirectory.split(/[/\\]/).at(-1) }}</p>
          <FButton variant="secondary" :disabled="busy" @click="saveDirectory = ''">{{ t("mapResources.removeSave") }}</FButton>
        </div>
        <p v-if="packagePath" class="filename" :title="packagePath">{{ packagePath.split(/[/\\]/).at(-1) }}</p>
        <div v-if="regions.length" class="field">
          <label :for="`${fieldId}-region`">{{ t("mapResources.region") }}</label>
          <FSelect
:id="`${fieldId}-region`" v-model="group" :options="regionOptions"
            :placeholder="t('mapResources.selectRegion')" :disabled="busy" />
        </div>
        <p v-if="error" class="error" role="alert">{{ t(`mapResources.${error}`) }}</p>
        <div class="source-actions">
          <FButton v-if="choosing" variant="secondary" :disabled="busy" @click="choosing = false">{{ t("mapResources.cancel") }}</FButton>
          <FButton :disabled="busy || !group" @click="openRegion">
            {{ t(busy ? "common.loading" : "mapResources.openRegion") }}
          </FButton>
        </div>
      </section>
    </div>
    <div v-else class="editor">
      <section class="map-column" :aria-label="t('mapResources.title')">
        <MapPreviewPane
v-model:active-resource="activeResource" :data="data" :title="title" :selected-plot-uid="selected"
          :options="previewOptions" :resource-editor="resourceEditor" @select-plot="selectCity" @resource-stroke="apply">
          <template #actions>
            <div class="map-actions">
              <FButton variant="secondary" size="sm" :disabled="busy" @click="choosing = true">{{ t("mapResources.changeSource") }}</FButton>
              <FButton size="sm" :disabled="busy || !dirty" @click="commit"><Save :size="13" aria-hidden="true" />{{ t("mapResources.save") }}</FButton>
            </div>
          </template>
        </MapPreviewPane>
        <p v-if="error" class="map-error error" role="alert">{{ t(`mapResources.${error}`) }}</p>
      </section>
      <aside class="inspector">
        <section class="inspector-section">
          <h3>
            <label v-if="available.length" :for="`${fieldId}-layer`">{{ t("mapResources.layer") }}</label>
            <template v-else>{{ t("mapResources.layer") }}</template>
          </h3>
          <template v-if="available.length">
            <FSelect
:id="`${fieldId}-layer`" :model-value="String(layerId)" :options="layerOptions"
              :disabled="busy" @update:model-value="layerId = Number($event)" />
            <div class="tools" role="group" :aria-label="t('mapResources.layer')">
              <FButton
                v-for="tool in visibleModes" :key="tool.key" size="sm" :aria-pressed="mode === tool.key"
                :variant="mode === tool.key ? 'primary' : 'secondary'"
                :disabled="busy || !resource || (eraseOnly && tool.key === 'paint')" @click="mode = tool.key">
                <component :is="isWind && tool.key === 'paint' ? ArrowRight : tool.icon" :size="13" aria-hidden="true" />
                {{ t(`mapResources.${isWind && tool.key === "paint" ? "placeWind" : tool.key}`) }}
              </FButton>
            </div>
            <div v-if="isWind" class="slider">
              <span>{{ t("mapResources.windAngle") }}</span>
              <output class="wind-angle">
                <ArrowRight :size="14" :style="{ transform: `rotate(${windArrowRotation(windAngle)}deg)` }" aria-hidden="true" />
                {{ t("mapResources.degrees", { value: n(windAngle) }) }}
              </output>
              <FSlider v-model="windAngle" :label="t('mapResources.windAngle')" :min="0" :max="359" :disabled="busy" />
            </div>
            <div v-else class="slider"><span>{{ t("mapResources.radius") }}</span><output>{{ t("mapResources.meters", { value: n(radius) }) }}</output>
              <FSlider v-model="radius" :label="t('mapResources.radius')" :min="8" :max="512" :step="8" :disabled="busy" />
            </div>
            <div v-if="!isWind && mode !== 'erase' && !eraseOnly" class="slider"><span>{{ t("mapResources.value") }}</span><output>{{ n(value) }}</output>
              <FSlider v-model="value" :label="t('mapResources.value')" :min="1" :max="65534" :disabled="busy" />
            </div>
            <p v-if="isWind || mode !== 'select'" class="hint">{{ t(isWind ? "mapResources.windHint" : "mapResources.brushHint") }}</p>
            <FButton class="undo" variant="secondary" size="sm" :disabled="busy || !schema?.strokes.length" @click="undo">
              <Undo2 :size="13" aria-hidden="true" />{{ t("mapResources.undo") }}
            </FButton>
          </template>
          <p v-else-if="activePlot" class="hint">{{ t("mapResources.unavailable") }}</p>
          <p v-else class="hint">{{ t("mapResources.chooseCity") }}</p>
        </section>
        <section class="inspector-section">
          <h3><label :for="`${fieldId}-city`">{{ t("mapResources.cityMetadata") }}</label></h3>
          <FSelect
:id="`${fieldId}-city`" :model-value="selected ?? ''" :options="cityOptions"
            :placeholder="t('mapResources.selectCity')" :disabled="busy || !cityOptions.length" @update:model-value="selectCity" />
          <template v-if="activePlot">
            <dl class="metadata">
              <dt>{{ t("mapResources.environment") }}</dt><dd>{{ activeCity?.environment ?? t("mapResources.unavailable") }}</dd>
              <dt>{{ t("mapResources.cityMetadata") }}</dt><dd>{{ activePlot.uid }}</dd>
              <dt>{{ t("mapResources.coordinates") }}</dt><dd>{{ n(activePlot.x) }}, {{ n(activePlot.y) }}</dd>
            </dl>
          </template>
          <span class="hint" role="status">{{ t(dirty ? "mapResources.unsaved" : "mapResources.saved") }}</span>
        </section>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.workbench { height: 100%; min-height: 0; background: var(--background); color: var(--foreground); }
.setup { display: grid; place-items: center; padding: 24px; height: 100%; overflow: auto; }
.source-panel { width: min(100%, 408px); display: grid; justify-items: start; gap: 13px; padding: 24px; border-radius: var(--radius-lg); background: var(--surface); border: 1px solid var(--border); }
.source-icon { color: var(--primary); }
.source-panel h2 { margin: 0; font-size: 20px; font-weight: 650; }
.source-panel p { color: var(--muted-foreground); margin: 0; font-size: 13px; overflow-wrap: anywhere; }
.source-actions { justify-self: end; display: flex; flex-wrap: wrap; gap: 8px; }
.source-buttons { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.field { display: grid; gap: 6px; width: 100%; font-size: 12px; }
.editor { height: 100%; display: grid; grid-template-columns: minmax(0, 1fr) minmax(205px, 258px); }
.map-column { position: relative; min-width: 0; min-height: 0; }
.map-actions { display: flex; flex-wrap: wrap; gap: 6px; }
.inspector { min-height: 0; overflow: auto; background: var(--surface); border-inline-start: 1px solid var(--border); }
.inspector-section { padding: 16px; display: grid; gap: 12px; border-bottom: 1px solid var(--border); font-size: 12px; }
.inspector h3 { margin: 0; font-size: 13px; font-weight: 650; }
.tools { display: flex; flex-wrap: wrap; gap: 6px; }
.undo { justify-self: start; }
.slider { display: grid; grid-template-columns: 1fr auto; gap: 5px; color: var(--muted-foreground); }
.slider .f-slider { grid-column: 1 / -1; }
.wind-angle { display: flex; align-items: center; gap: 6px; }
.hint { margin: 0; color: var(--muted-foreground); }
.metadata { display: grid; grid-template-columns: 1fr auto; gap: 7px; margin: 0; color: var(--muted-foreground); }
.metadata dd { margin: 0; text-align: right; }
.source-panel .error, .error { color: var(--danger); }
.map-error { position: absolute; bottom: 56px; left: 12px; margin: 0; padding: 8px 12px; background: var(--surface); border: 1px solid var(--danger); border-radius: var(--radius-sm); }
@media (max-width: 760px) {
  .editor { grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(240px, 2fr) minmax(130px, 1fr); overflow: auto; }
  .inspector { border-inline-start: 0; border-top: 1px solid var(--border); }
}
</style>
