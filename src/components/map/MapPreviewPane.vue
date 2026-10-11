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
const props = defineProps<{
  data: Region3DData | null;
  title?: string;
  options?: { kind: string; label: string; available: boolean }[];
}>();
const emit = defineEmits<{ selectPlot: [uid: string] }>();
const { t } = useI18n();
const fullscreen = ref(false),
  activeResource = ref<string | null>(null),
  opacity = ref(0.75);
const plots = ref(true),
  vegetation = ref(true),
  water = ref(true),
  roads = ref(true);
const layers = computed(() => ({
  showPlots: plots.value,
  showVegetation: vegetation.value,
  showWater: water.value,
  showRoads: roads.value,
  activeResource: activeResource.value,
  resourceOpacity: opacity.value,
}));
const options = computed(() =>
  (
    props.options ??
    ["soil", "waterTable", "forest"].map((kind) => ({
      kind,
      available: true,
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
    <header>
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
    <div class="viewport">
      <MapViewer3D
        v-if="!fullscreen"
        :data="data"
        v-bind="layers"
        @select-plot="emit('selectPlot', $event)"
      />
      <FEmpty
        v-if="!data"
        class="empty"
        icon-name="Map"
        variant="compact"
        :title="t('studio.map.emptyTitle')"
        :desc="t('studio.map.emptyDescription')"
      />
    </div>
    <FSheet v-model:open="fullscreen" width="100vw">
      <section class="fullscreen">
        <header>
          <strong>{{ title ?? t("studio.map.title") }}</strong
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
            v-bind="layers"
            @select-plot="emit('selectPlot', $event)"
          />
        </div>
      </section>
    </FSheet>
    <Teleport to="body" :disabled="!fullscreen">
      <aside :class="['layers', { floating: fullscreen }]">
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
      </aside>
    </Teleport>
  </section>
</template>
<style scoped>
.map-preview-pane {
  display: flex;
  flex-direction: column;
  min-height: 480px;
  height: 100%;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  overflow: hidden;
}
header {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  align-items: center;
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}
header strong {
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
  min-height: 400px;
}
.empty {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
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
  gap: 12px;
  flex-wrap: wrap;
  padding: 10px;
  background: var(--surface);
  color: var(--foreground);
  border-top: 1px solid var(--border);
  font-size: 12px;
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
</style>
