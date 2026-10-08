<script setup lang="ts">
import FDropdownSelect from "@/components/ui/FSelect.vue";
import {
  defineAsyncComponent,
  shallowRef,
  watch,
  onMounted,
  nextTick,
} from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import type { Region3DData, RegionSummary } from "@/lib/region-map";
const MapViewer3D = defineAsyncComponent(
  () => import("@/components/map/MapViewer3D.vue"),
);
const packagePath = defineModel<string>("packagePath", { required: true });
const group = defineModel<string>("group", { required: true });
defineProps<{ disabled: boolean }>();
const emit = defineEmits<{ resize: [] }>();
const { t, locale } = useI18n();
const regions = shallowRef<RegionSummary[]>([]);
const preview = shallowRef<Region3DData | null>(null);
const busy = shallowRef(false);
const error = shallowRef("");
let request = 0;
onMounted(async () => {
  if (!packagePath.value) return;
  const path = packagePath.value;
  try {
    const result = await invoke<RegionSummary[]>("map_panel_list_regions", {
      packagePath: path,
    });
    if (path === packagePath.value) regions.value = result;
  } catch {
    /* A manually entered path remains editable. */
  }
});
watch([packagePath, group], () => {
  request++;
  preview.value = null;
  busy.value = false;
});
async function choose() {
  const path = await open({
    multiple: false,
    filters: [{ name: ".package", extensions: ["package"] }],
  });
  if (typeof path !== "string") return;
  packagePath.value = path;
  try {
    regions.value = await invoke("map_panel_list_regions", {
      packagePath: path,
    });
    group.value = regions.value[0]?.group ?? "";
  } catch (cause) {
    error.value = String(cause);
  }
}
async function render() {
  const token = ++request;
  busy.value = true;
  error.value = "";
  try {
    const data = await invoke<Region3DData>("map_panel_region_3d", {
      packagePath: packagePath.value,
      group: group.value,
      saveDirectory: null,
    });
    if (token === request) {
      preview.value = data;
      await nextTick();
      emit("resize");
    }
  } catch (cause) {
    if (token === request) error.value = String(cause);
  } finally {
    if (token === request) busy.value = false;
  }
}
</script>
<template>
  <div class="map-source-tool">
    <button type="button" :disabled="disabled || busy" @click="choose">
      {{ t("flow.choosePackage") }}
    </button>
    <FDropdownSelect
      v-if="regions.length"
      v-model="group"
      :disabled="disabled || busy"
      :options="
        regions.map((region) => ({
          value: region.group,
          label:
            (locale === 'en-US' ? region.displayNameEn : region.displayName) ||
            region.group,
        }))
      "
    />
    <button
      type="button"
      :disabled="disabled || busy || !packagePath || !group"
      @click="render"
    >
      {{ t(busy ? "flow.busy" : "flow.previewSource") }}
    </button>
    <p v-if="error" role="alert">{{ error }}</p>
    <div v-if="preview" class="map-preview">
      <MapViewer3D :data="preview" />
    </div>
  </div>
</template>
<style scoped>
.map-source-tool {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.map-source-tool > button {
  min-height: 26px;
  padding: 0 8px;
  font-size: 11px;
  cursor: pointer;
  color: inherit;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 6px;
}
.map-preview {
  height: 260px;
  min-height: 260px;
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
}
.map-preview :deep(.viewer3d) {
  min-height: 0;
}
.map-source-tool p {
  font-size: 12px;
  overflow-wrap: anywhere;
}
</style>
