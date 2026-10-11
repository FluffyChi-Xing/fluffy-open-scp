<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import FSelect from "@/components/ui/FSelect.vue";
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { isTauri, tauriApi } from "@/api";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { base64ToRgba } from "@/lib/raster-editor/encoders";
import {
  parseRasterSchema,
  validateParameters,
  validateSize,
  type ParameterTable,
  type RasterProfile,
  type RasterSchema,
  type RasterSource,
} from "@/lib/raster-editor/schema";
import {
  catalogKey,
  type RasterCatalogEntry,
} from "@/lib/raster-editor/catalog";
import type { DecalEntryMeta, PropertyDocumentSummary, Tgi } from "@/api/tauri";
const props = defineProps<{
  profile: RasterProfile;
  disabled?: boolean;
  imageWidth?: number;
  imageHeight?: number;
}>();
const emit = defineEmits<{
  create: [width: number, height: number];
  loadLot: [summary: PropertyDocumentSummary & { packageId: number }];
  loadDecal: [entry: DecalEntryMeta, source: RasterSource];
  image: [
    image: { width: number; height: number; pixels: Uint8ClampedArray },
    source: RasterSource,
  ];
  schema: [schema: RasterSchema];
  parameters: [value: ParameterTable];
}>();
const packages = useGamePackagesStore();
const width = ref(64),
  height = ref(64),
  packageId = ref<number | null>(null);
const entries = ref<(RasterCatalogEntry & { name: string })[]>([]);
watch(
  () => [props.imageWidth, props.imageHeight],
  ([w, h]) => {
    if (w && h) {
      width.value = w;
      height.value = h;
    }
  },
  { immediate: true },
);
const query = ref(""),
  busy = ref(false),
  error = ref(""),
  selected = ref("");

const visible = computed(() =>
  entries.value.filter((e) =>
    e.name.toLowerCase().includes(query.value.toLowerCase()),
  ),
);
let epoch = 0;
onBeforeUnmount(() => {
  epoch++;
});
const key = (t: Tgi) =>
  [t.typeId, t.group, t.instance]
    .map((n) => n.toString(16).padStart(8, "0"))
    .join(":");
async function list() {
  const generation = ++epoch;
  entries.value = [];
  selected.value = "";
  if (packageId.value == null) {
    busy.value = false;
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    const rows = await tauriApi.raster.catalog(packageId.value, props.profile);
    if (generation !== epoch) return;
    entries.value = rows.map((e) => ({
      ...e,
      name: `${key(e.tgi)} · ${key(e.owner)}${e.entryIndex == null ? "" : " #" + e.entryIndex}`,
    }));
  } catch (e) {
    if (generation === epoch) error.value = String(e);
  } finally {
    if (generation === epoch) busy.value = false;
  }
}
watch(packageId, () => {
  void list();
});
watch(
  () => props.profile,
  () => {
    query.value = "";
    void list();
  },
);
watch(
  () => packages.opened.map((p) => p.package.packageId),
  (ids) => {
    if (packageId.value == null || !ids.includes(packageId.value))
      packageId.value = ids[0] ?? null;
    else void list();
  },
  { immediate: true },
);
async function copy() {
  const entry = entries.value.find((e) => catalogKey(e) === selected.value),
    id = packageId.value;
  if (!entry || id == null) return;
  const generation = ++epoch;
  busy.value = true;
  error.value = "";
  try {
    if (props.profile === "lot-mask") {
      emit("loadLot", {
        packageId: entry.ownerPackageId,
        tgi: entry.owner,
        lotMask: entry.tgi,
        lotMaskResolved: entry.tgi,
        lotMaskPackageId: entry.packageId,
        lotSize: null,
        entryCount: null,
        materialInstance: null,
      });
      return;
    }
    if (props.profile.startsWith("decal-") && entry.entryIndex != null) {
      const dictionary = await tauriApi.packages.readDecalDictionary(
        entry.ownerPackageId,
        entry.owner,
      );
      if (generation !== epoch) return;
      const item = dictionary.entries.find((e) => e.index === entry.entryIndex);
      if (item)
        emit(
          "loadDecal",
          { ...item, raster: entry.tgi },
          {
            name: key(entry.owner),
            packageId: entry.ownerPackageId,
            tgi: entry.owner,
          },
        );
      return;
    }
    const data = await tauriApi.raster.readSource(
      id,
      entry.tgi,
      props.profile === "slot0",
    );
    if (generation !== epoch) return;
    if (data.parameters) {
      emit("parameters", validateParameters(data.parameters));
      return;
    }
    if (!data.rgbaBase64) throw new Error(t("rasterEditor.text157"));
    emit(
      "image",
      {
        width: data.width,
        height: data.height,
        pixels: base64ToRgba(data.rgbaBase64),
      },
      { name: entry.name, packageId: id, tgi: { ...entry.tgi } },
    );
  } catch (e) {
    if (generation === epoch) error.value = String(e);
  } finally {
    if (generation === epoch) busy.value = false;
  }
}
async function importImage() {
  busy.value = true;
  error.value = "";
  const generation = ++epoch;
  try {
    const path = await tauriApi.workspace.pickImageFile(
      t("rasterEditor.text085"),
    );
    if (!path) return;
    const image = await tauriApi.raster.readImageRgba(path);
    if (generation !== epoch) return;
    emit(
      "image",
      { ...image, pixels: base64ToRgba(image.rgbaBase64) },
      { name: path.split(/[\\/]/).pop() ?? path },
    );
  } catch (e) {
    if (generation === epoch) error.value = String(e);
  } finally {
    if (generation === epoch) busy.value = false;
  }
}
async function importJson(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  error.value = "";
  try {
    if (file.size > 96 * 1024 * 1024)
      throw new Error(t("rasterEditor.text086"));
    const value = JSON.parse(await file.text());
    if (value.format === "openscp.raster")
      emit("schema", parseRasterSchema(value));
    else if (props.profile === "slot0")
      emit("parameters", validateParameters(value));
    else throw new Error(t("rasterEditor.text087"));
  } catch (e) {
    error.value = String(e);
  }
  input.value = "";
}
function create() {
  try {
    validateSize(width.value, height.value);
    emit("create", width.value, height.value);
    error.value = "";
  } catch (e) {
    error.value = String(e);
  }
}
</script>
<template>
  <section class="source-panel">
    <h3>{{ t("rasterEditor.text088") }}</h3>
    <fieldset :disabled="disabled || busy">
      <div class="package-source">
        <div v-if="profile !== 'slot0'" class="dimensions">
          <label>
            {{ t("rasterEditor.text089") }}
            <input
              v-model.number="width"
              type="number"
              min="1"
              max="4096" /></label
          ><span>×</span
          ><label>
            {{ t("rasterEditor.text090") }}
            <input v-model.number="height" type="number" min="1" max="4096"
          /></label>
        </div>
        <button v-if="profile !== 'slot0'" @click="create">
          {{ t("rasterEditor.text091") }}
        </button>
        <button v-if="isTauri() && profile !== 'slot0'" @click="importImage">
          {{ t("rasterEditor.text092") }}
        </button>
        <div class="package-source">
          <p v-if="!isTauri()">{{ t("rasterEditor.nativePackages") }}</p>
          <p v-else-if="!packages.opened.length">
            {{ t("rasterEditor.noPackages") }}
          </p>
          <label>
            {{ t("rasterEditor.text093") }}
            <FSelect
              :disabled="disabled || busy"
              :model-value="packageId == null ? '' : String(packageId)"
              :options="[
                { value: '', label: t('rasterEditor.text094') },
                ...packages.opened.map((p) => ({
                  value: String(p.package.packageId),
                  label: p.package.path.split(/[\\/]/).pop() ?? p.package.path,
                })),
              ]"
              @update:model-value="packageId = $event ? Number($event) : null"
          /></label>
          <label>
            {{ t("rasterEditor.text095") }}
            <input v-model="query" :placeholder="t('rasterEditor.sourceSearch')"
          /></label>
          <FSelect
            v-model="selected"
            :disabled="disabled || busy"
            :options="[
              { value: '', label: t('rasterEditor.text096') },
              ...visible.map((e) => ({ value: catalogKey(e), label: e.name })),
            ]"
          />
          <button :disabled="!selected" @click="copy">
            {{ t("rasterEditor.text098") }}
          </button>
        </div>
      </div>
      <label class="json-import"
        >{{
          profile === "slot0"
            ? t("rasterEditor.text099")
            : t("rasterEditor.text100")
        }}<input
          type="file"
          accept=".json,application/json"
          @change="importJson"
      /></label>
    </fieldset>
    <p>{{ t("rasterEditor.filteredSources") }}</p>
    <p v-if="!busy && !entries.length">
      {{ t("rasterEditor.noMatchingSources") }}
    </p>
    <p v-if="busy" role="status">{{ t("rasterEditor.text101") }}</p>
    <p v-if="error" role="alert">{{ error }}</p>
  </section>
</template>
<style scoped>
.source-panel {
  padding: 16px;
}
h3 {
  font-size: 12px;
  margin: 0 0 12px;
}
fieldset,
.package-source {
  border: 0;
  padding: 0;
  min-width: 0;
  display: grid;
  gap: 10px;
}
label {
  display: grid;
  gap: 5px;
  font-size: 11px;
  color: var(--muted-foreground);
}
input,
select,
button {
  min-width: 0;
  width: 100%;
  padding: 7px;
  background: var(--surface);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: 4px;
  font-size: 11px;
}
select[size] {
  font-family: monospace;
  padding: 2px;
}
option {
  padding: 5px;
}
.dimensions {
  display: flex;
  gap: 7px;
  align-items: end;
}
.dimensions label {
  flex: 1;
  min-width: 0;
}
button {
  cursor: pointer;
}
button:disabled,
fieldset:disabled {
  opacity: 0.5;
}
p {
  font-size: 11px;
}
[role="alert"] {
  color: var(--destructive);
  overflow-wrap: anywhere;
}
</style>
