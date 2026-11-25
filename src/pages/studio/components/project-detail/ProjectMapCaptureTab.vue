<script setup lang="ts">
import { computed, shallowRef, useId } from "vue";
import { useI18n } from "vue-i18n";
import FButton from "@/components/ui/FButton.vue";
import FSelect from "@/components/ui/FSelect.vue";
import LogStream from "@/components/runtime-observer/LogStream.vue";
import { useAssetCapture } from "./useAssetCapture";

const props = defineProps<{ project: string }>();
const { t, te } = useI18n();
const { processes, pid, status, busy, loading, error, epoch, scan, start, stop } =
  useAssetCapture(props.project, "map");
const processId = useId();
const layerId = useId();
const mapLayer = shallowRef("");
const running = computed(() => Boolean(status.value?.running));
const selection = computed({
  get: () => pid.value?.toString() ?? "",
  set: (value) => { pid.value = value ? Number(value) : undefined; },
});
const processOptions = computed(() => processes.value.map(({ name, pid }) => ({
  value: String(pid), label: `${name} · ${pid}`,
})));
const layerOptions = computed(() => [
  { value: "", label: t("assetCapture.filter.all") },
  ...["coal", "ore", "oil", "cityWaterTable", "wind", "radiation", "groundPollution"].map((value) => ({
    value, label: t(`mapResources.${value}`),
  })),
]);
const summary = computed(() => {
  if (loading.value) return t("assetCapture.loading");
  if (!processes.value.length) return t("assetCapture.noProcess");
  if (status.value?.status?.ready) return t("observerMenu.mapStatus", { tracked: status.value.status.tracked });
  return t(running.value ? "observerMenu.mapWaiting" : "observerMenu.mapHint");
});
const failure = computed(() => {
  const detail = error.value || status.value?.status?.error;
  if (!detail) return "";
  const key = `assetCapture.errors.${detail}`;
  return te(key) ? t(key) : t("assetCapture.errors.failed", { detail });
});
</script>

<template>
  <section class="map-capture" :aria-label="t('observerMenu.mapResources')">
    <div class="controls">
      <div class="field">
        <label :for="processId">{{ t("assetCapture.process") }}</label>
        <FSelect :id="processId" v-model="selection" :options="processOptions"
          :placeholder="t('assetCapture.noProcess')" :disabled="busy || running || loading || !processes.length" />
      </div>
      <div class="field">
        <label :for="layerId">{{ t("mapResources.layer") }}</label>
        <FSelect :id="layerId" v-model="mapLayer" :options="layerOptions" />
      </div>
      <FButton variant="secondary" :disabled="busy || running || loading" @click="scan">{{ t("assetCapture.refresh") }}</FButton>
      <FButton v-if="running" variant="secondary" :disabled="busy" @click="stop">{{ t("runtimeObserver.stop") }}</FButton>
      <FButton v-else :disabled="busy || loading || !pid" @click="start()">{{ t("runtimeObserver.start") }}</FButton>
    </div>
    <p v-if="failure" class="error" role="alert">{{ failure }}</p>
    <p v-else class="status" role="status">{{ summary }}</p>
    <LogStream :key="epoch" :rows="status?.recent ?? []" :map-layer="mapLayer" />
  </section>
</template>

<style scoped>
.map-capture { display: grid; gap: 12px; min-inline-size: 0; padding-block: 16px; }
.controls { display: flex; flex-wrap: wrap; align-items: end; gap: 8px; }
.field { display: grid; gap: 4px; min-inline-size: 0; inline-size: min(240px, 100%); font-size: 12px; }
.status, .error { margin: 0; font-size: 12px; }
.status { color: var(--muted-foreground); }
.error { color: var(--destructive); }
</style>
