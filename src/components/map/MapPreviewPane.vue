<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import MapViewer3D from "./MapViewer3D.vue";
import MapResourceControls from "./MapResourceControls.vue";
import FSheet from "@/components/ui/FSheet.vue";
import { Switch } from "@/components/ui/switch";
import FIcon from "@/components/extensions/FIcon.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import type { Region3DData } from "@/lib/region-map";
import { hasStateResource } from "./map-state-fields";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import TerrainBrushToolbar from "./TerrainBrushToolbar.vue";
import type { TerrainBrushMode, TerrainStroke } from "./terrain-brush";
const props = defineProps<{
  data: Region3DData | null;
  title?: string;
  emptyHint?: string;
  selectedPlotUid?: string | null;
  options?: { kind: string; label: string; available: boolean }[];
  terrainSource?: { packagePath: string; group: string } | null;
}>();
const emit = defineEmits<{ selectPlot: [uid: string] }>();
const { t } = useI18n();
const mode = ref<TerrainBrushMode | null>(null);
const radius = ref(256);
const strength = ref(64);
const strokes = ref<TerrainStroke[]>([]);
const editedHeight = ref<string | null>(null);
const busy = ref(false);
const statusKey = ref("");
const exportedTiles = ref(0);
let editGeneration = 0;

watch(() => props.data, () => {
  ++editGeneration;
  mode.value = null;
  strokes.value = [];
  editedHeight.value = null;
  statusKey.value = "";
  busy.value = false;
});

async function preview(next: TerrainStroke[]) {
  if (!props.terrainSource || busy.value) return;
  if (!next.length) {
    strokes.value = [];
    editedHeight.value = null;
    statusKey.value = "";
    return;
  }
  const generation = editGeneration;
  busy.value = true;
  statusKey.value = "working";
  try {
    const result = await invoke<{ heightPngBase64: string }>("map_panel_apply_terrain", {
      ...props.terrainSource, strokes: next,
    });
    if (generation !== editGeneration) return;
    editedHeight.value = result.heightPngBase64;
    strokes.value = next;
    statusKey.value = "";
  } catch (error) {
    console.error(error);
    if (generation === editGeneration) statusKey.value = "previewFailed";
  } finally {
    if (generation === editGeneration) busy.value = false;
  }
}

function stamp(x: number, y: number) {
  if (mode.value && strokes.value.length < 256) {
    void preview([...strokes.value, { mode: mode.value, x, y, radius: radius.value, strength: strength.value }]);
  } else if (mode.value && !busy.value) {
    statusKey.value = "limit";
  }
}

async function exportTerrain() {
  const source = props.terrainSource;
  if (!source || busy.value || !strokes.value.length) return;
  const generation = editGeneration;
  busy.value = true;
  statusKey.value = "";
  try {
    const outPath = await save({
      defaultPath: `000_OpenSCP_Terrain_${source.group}.package`,
      filters: [{ name: t("studio.map.brush.packageFile"), extensions: ["package"] }],
    });
    if (!outPath || generation !== editGeneration) return;
    statusKey.value = "working";
    const result = await invoke<{ entryCount: number }>("map_panel_apply_terrain", {
      ...source, strokes: strokes.value, outPath,
    });
    if (generation !== editGeneration) return;
    exportedTiles.value = result.entryCount;
    statusKey.value = result.entryCount ? "exported" : "noChanges";
  } catch (error) {
    console.error(error);
    if (generation === editGeneration) statusKey.value = "exportFailed";
  } finally {
    if (generation === editGeneration) busy.value = false;
  }
}

const brushStatus = computed(() => statusKey.value
  ? t(`studio.map.brush.${statusKey.value}`, { count: exportedTiles.value })
  : "");
const fullscreen = ref(false),
  activeResource = ref<string | null>(null),
  opacity = ref(0.75);
const plots = ref(true),
  vegetation = ref(true),
  water = ref(true),
  roads = ref(true);
const layers = computed(() => ({
  showPlots: plots.value,
  selectedPlotUid: props.selectedPlotUid,
  showVegetation: vegetation.value,
  showWater: water.value,
  showRoads: roads.value,
  activeResource: activeResource.value,
  resourceOpacity: opacity.value,
}));
const options = computed(() =>
  (
    props.options ??
    ["soil", "waterTable", "cityWaterTable", "forest"].map((kind) => ({
      kind,
      available: kind !== "cityWaterTable" || hasStateResource(props.data, kind),
      label: t(`studio.map.res${kind[0]!.toUpperCase() + kind.slice(1)}`),
    }))
  ).filter(
    (option) =>
      !["desirability", "desirabilitytwo"].includes(option.kind.toLowerCase()),
  ),
);
watch(options, () => {
  if (
    !options.value.some(
      (option) => option.kind === activeResource.value && option.available,
    )
  )
    activeResource.value = null;
});
</script>
<template>
  <section class="map-preview-pane nodrag nopan nowheel">
    <header v-if="data" class="viewport-bar">
      <strong class="region-title">{{ title || t('studio.map.title') }}</strong>
      <MapResourceControls
        v-model="activeResource"
        v-model:opacity="opacity"
        :options="options"
        :disabled="!data"
      />
      <button
        :disabled="!data"
        :aria-label="t('studio.map.fullscreen')"
        @click="fullscreen = true"
      >
        <FIcon name="Expand" :size="15" />
      </button>
    </header>
    <div class="viewport" :class="{ 'viewport-empty': !data }">
      <MapViewer3D
        v-if="data && !fullscreen"
        :data="data"
        :edited-height="editedHeight"
        :terrain-editing="mode !== null"
        :brush-radius="radius"
        v-bind="layers"
        @select-plot="emit('selectPlot', $event)"
        @terrain-point="stamp"
      />
      <TerrainBrushToolbar
        v-if="!fullscreen && data && terrainSource"
        v-model:mode="mode" v-model:radius="radius" v-model:strength="strength"
        :busy="busy" :can-undo="strokes.length > 0" :status="brushStatus"
        @undo="preview(strokes.slice(0, -1))" @export="exportTerrain"
      />
      <FEmpty
        v-if="!data"
        class="empty"
        icon-name="Map"
        variant="compact"
        :title="t('studio.map.emptyTitle')"
        :desc="emptyHint ?? t('studio.map.emptyDescription')"
      />
    </div>
    <FSheet v-model:open="fullscreen" width="100vw">
      <section class="fullscreen">
        <header class="viewport-bar">
          <strong class="region-title">{{ title || t("studio.map.title") }}</strong
          ><MapResourceControls
            v-model="activeResource"
            v-model:opacity="opacity"
            :options="options"
            :disabled="!data"
          /><button :aria-label="t('shell.close')" @click="fullscreen = false">
            <FIcon name="X" :size="15" />
          </button>
        </header>
        <div class="viewport">
          <MapViewer3D
            v-if="fullscreen"
            :data="data"
            :edited-height="editedHeight"
            :terrain-editing="mode !== null"
            :brush-radius="radius"
            v-bind="layers"
            @select-plot="emit('selectPlot', $event)"
            @terrain-point="stamp"
          />
          <TerrainBrushToolbar
            v-if="fullscreen && data && terrainSource"
            v-model:mode="mode" v-model:radius="radius" v-model:strength="strength"
            :busy="busy" :can-undo="strokes.length > 0" :status="brushStatus"
            @undo="preview(strokes.slice(0, -1))" @export="exportTerrain"
          />
        </div>
      </section>
    </FSheet>
    <Teleport to="body" :disabled="!fullscreen">
      <aside v-if="data" :class="['layers', { floating: fullscreen }]">
        <label
          ><Switch v-model="plots" :aria-label="t('studio.map.layerPlots')" />{{ t("studio.map.layerPlots") }}</label
        >
        <label
          ><Switch v-model="vegetation" :aria-label="t('studio.map.layerVegetation')" />{{
            t("studio.map.layerVegetation")
          }}</label
        >
        <label
          ><Switch v-model="water" :aria-label="t('studio.map.layerWater')" />{{ t("studio.map.layerWater") }}</label
        >
        <label
          ><Switch v-model="roads" :aria-label="t('studio.map.layerRoads')" />{{ t("studio.map.layerRoads") }}</label
        >
        <span class="camera-hint">{{ t('studio.map.cameraHint') }}</span>
      </aside>
    </Teleport>
  </section>
</template>
<style scoped>
.map-preview-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  overflow: hidden;
}
.viewport-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 14px;
  justify-content: flex-end;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}
.region-title {
  font-size: .8125rem;
  overflow-wrap: anywhere;
  margin-inline-end: auto;
}
button {
  display: grid;
  place-items: center;
  min-width: 28px;
  min-height: 28px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--foreground);
  cursor: pointer;
}
.viewport {
  position: relative;
  flex: 1;
  min-height: 0;
}
.viewport :deep(.viewer3d) { min-height: 0; }
.viewport-empty {
  background-color: var(--surface);
  background-image:
    linear-gradient(to right, color-mix(in srgb, var(--brand) 12%, transparent) 1px, transparent 1px),
    linear-gradient(to bottom, color-mix(in srgb, var(--brand) 12%, transparent) 1px, transparent 1px);
  background-size: 32px 32px;
  background-position: center;
}
.empty {
  position: absolute;
  inset-block-start: 50%;
  inset-inline-start: 50%;
  translate: -50% -50%;
  inline-size: min(25rem, calc(100% - 2rem));
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-md);
}
.fullscreen {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--surface);
  color: var(--foreground);
}
.layers {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  padding: 10px;
  background: var(--surface);
  color: var(--foreground);
  border-top: 1px solid var(--border);
  font-size: 12px;
}
.camera-hint {
  margin-inline-start: auto;
  color: var(--muted-foreground);
}
.layers label {
  display: flex;
  align-items: center;
  gap: 6px;
}
.layers.floating {
  position: fixed;
  z-index: 200;
  right: 24px;
  top: 110px;
  display: grid;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}
.layers.floating .camera-hint { display: none; }
</style>
