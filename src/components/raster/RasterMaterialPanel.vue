<script setup lang="ts">
import { PROFILE_BEHAVIOR } from "@/lib/raster-editor/profileBehavior";
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { matchesDecalProfile } from "@/lib/raster-editor/catalog";
import FSelect from "@/components/ui/FSelect.vue";
import { Switch } from "@/components/ui/switch";
import { computed, ref, watch } from "vue";
import { isTauri, tauriApi } from "@/api";
import { useGamePackagesStore } from "@/stores/gamePackages";
import type { DecalEntryMeta, PropertyDocumentSummary, Tgi } from "@/api/tauri";
import type {
  RasterProfile,
  RasterSchema,
  RasterSource,
} from "@/lib/raster-editor/schema";
const props = defineProps<{ profile: RasterProfile; disabled?: boolean }>();
const lot = defineModel<RasterSchema["lot"]>("lot", { required: true });
const decal = defineModel<RasterSchema["decal"]>("decal", { required: true });
const sampling = defineModel<RasterSchema["sampling"]>("sampling", {
  required: true,
});
const emit = defineEmits<{
  loadLot: [summary: PropertyDocumentSummary & { packageId: number }];
  loadDecal: [entry: DecalEntryMeta, atlas: RasterSource];
  changed: [];
}>();
const packages = useGamePackagesStore();
const documents = ref<(PropertyDocumentSummary & { packageId: number })[]>([]);
const busy = ref(false),
  error = ref(""),
  selected = ref("");
const entries = ref<DecalEntryMeta[]>([]),
  selectedEntry = ref<number | null>(null);
const isDecal = computed(() => props.profile.startsWith("decal-"));
let generation = 0;
const key = (id: number, tgi: Tgi) => `${id}:${tgi.group}:${tgi.instance}`;
const documentQuery = ref(""),
  entryQuery = ref("");
const documentOptions = computed(() => [
  { value: "", label: t("rasterEditor.text045") },
  ...documents.value
    .filter((d) =>
      d.tgi.instance
        .toString(16)
        .toLowerCase()
        .includes(documentQuery.value.toLowerCase().replace(/^0x/, "")),
    )
    .slice(0, 300)
    .map((d) => ({
      value: key(d.packageId, d.tgi),
      label: t("rasterEditor.text046", {
        p0: d.packageId,
        p1: d.tgi.instance.toString(16),
      }),
    })),
]);
const entryOptions = computed(() => [
  { value: "", label: t("rasterEditor.text047") },
  ...entries.value
    .filter((e) =>
      e.id?.instance
        .toString(16)
        .toLowerCase()
        .includes(entryQuery.value.toLowerCase().replace(/^0x/, "")),
    )
    .slice(0, 300)
    .map((e) => ({
      value: String(e.index),
      label: `${e.index} · 0x${e.id?.instance.toString(16)}`,
    })),
]);
async function scan() {
  const epoch = ++generation;
  busy.value = true;
  error.value = "";
  try {
    const kind = isDecal.value ? "decal" : "lot";
    const results = await Promise.all(
      packages.opened.map(async (p) =>
        (
          await tauriApi.raster.listPropertyDocuments(p.package.packageId, kind)
        ).map((d) => ({ ...d, packageId: p.package.packageId })),
      ),
    );
    if (epoch === generation)
      documents.value = results
        .flat()
        .filter(
          (d) =>
            !isDecal.value ||
            matchesDecalProfile(props.profile, d.materialInstance),
        );
  } catch (e) {
    if (epoch === generation) error.value = String(e);
  } finally {
    if (epoch === generation) busy.value = false;
  }
}
watch(
  () => props.profile,
  () => {
    generation++;
    documents.value = [];
    entries.value = [];
    selectedEntry.value = null;
    selected.value = "";
    busy.value = false;
  },
);
async function select() {
  const d = documents.value.find(
    (d) => key(d.packageId, d.tgi) === selected.value,
  );
  if (!d) return;
  if (isDecal.value) {
    const epoch = ++generation;
    busy.value = true;
    error.value = "";
    entries.value = [];
    selectedEntry.value = null;
    try {
      const dictionary = await tauriApi.packages.readDecalDictionary(
        d.packageId,
        d.tgi,
      );
      if (epoch !== generation) return;
      decal.value = {
        ...decal.value,
        atlas: {
          name: selected.value,
          packageId: d.packageId,
          tgi: { ...d.tgi },
        },
      };
      entries.value = dictionary.entries;
      emit("changed");
    } catch (e) {
      if (epoch === generation) error.value = String(e);
    } finally {
      if (epoch === generation) busy.value = false;
    }
  } else emit("loadLot", d);
}
function loadEntry() {
  const entry = entries.value.find((e) => e.index === selectedEntry.value);
  if (entry && decal.value.atlas) emit("loadDecal", entry, decal.value.atlas);
}
const label = (s: RasterSource | null) =>
  s?.tgi
    ? `0x${s.tgi.instance.toString(16).toUpperCase()}`
    : t("rasterEditor.text048");
function colorHex(c: number[]) {
  return (
    "#" +
    c
      .slice(0, 3)
      .map((n) => n.toString(16).padStart(2, "0"))
      .join("")
  );
}
function setLotColor(i: number, hex: string) {
  const colors = lot.value.colors.map(
    (c) => [...c] as [number, number, number, number],
  );
  colors[i] = [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
    colors[i][3],
  ];
  lot.value = { ...lot.value, colors };
  emit("changed");
}
function setTile(i: number, tile: number) {
  const colors = lot.value.colors.map(
    (c) => [...c] as [number, number, number, number],
  );
  colors[i][3] = tile;
  lot.value = { ...lot.value, colors };
  emit("changed");
}
</script>
<template>
  <section class="material-panel">
    <h3>{{ t("rasterEditor.text049") }}</h3>
    <fieldset :disabled="disabled" @change="emit('changed')">
      <label
        ><Switch v-model="sampling.generateMips" :aria-label="t('rasterEditor.text050')" />
        {{ t("rasterEditor.text050") }}
      </label>
      <p>{{ t("rasterEditor.text051") }}</p>
      <template v-if="profile === 'lot-mask'">
        <label>
          {{ t("rasterEditor.text052") }}
          <input
            v-model.number="sampling.metersPerPixel"
            type="number"
            min="0.001"
            step="0.01"
        /></label>
        <div v-for="(c, i) in lot.colors" :key="i" class="color-row">
          <label
            >LC{{ i + 1
            }}<input
              :disabled="!!lot.target"
              :value="colorHex(c)"
              type="color"
              @input="
                setLotColor(i, ($event.target as HTMLInputElement).value)
              " /></label
          ><label>
            {{ t("rasterEditor.text053") }}
            <input
              :disabled="!!lot.target"
              :value="c[3]"
              type="number"
              min="0"
              max="15"
              @change="
                setTile(i, Number(($event.target as HTMLInputElement).value))
              "
          /></label>
        </div>
        <p v-if="lot.target">{{ t("rasterEditor.text054") }}</p>
        <p>{{ t("rasterEditor.text055") }} {{ label(lot.target) }}</p>
        <button
          v-if="lot.target"
          @click="
            lot = { ...lot, target: null };
            emit('changed');
          "
        >
          {{ t("rasterEditor.text056") }}
        </button>
      </template>
      <template v-if="isDecal">
        <div v-if="PROFILE_BEHAVIOR[profile].palette" class="colors">
          <label v-for="(c, i) in decal.colors" :key="i"
            >{{ t("rasterEditor.decalColor", { n: i + 1 })
            }}<input
              :value="c"
              type="color"
              @input="
                decal = {
                  ...decal,
                  colors: decal.colors.map((v, j) =>
                    i === j ? ($event.target as HTMLInputElement).value : v,
                  ),
                };
                emit('changed');
              "
          /></label>
        </div>
        <label>
          {{ t("rasterEditor.text057") }}
          <input
            v-model.number="decal.aspectRatio"
            type="number"
            min="0.0001"
            step="0.01"
        /></label>
        <label>
          {{ t("rasterEditor.text058") }}
          <input v-model="decal.entryId" placeholder="0x12345678"
        /></label>
        <label>
          {{ t("rasterEditor.text059") }}
          <input v-model="decal.replaceInstance" placeholder="0x12345678"
        /></label>
        <template v-if="entries.length"
          ><label>
            {{ t("rasterEditor.text060") }}
            <input
              v-model="entryQuery"
              :placeholder="t('rasterEditor.text061')" /></label
          ><FSelect
            :disabled="disabled || busy"
            :model-value="selectedEntry == null ? '' : String(selectedEntry)"
            :options="entryOptions"
            @update:model-value="selectedEntry = $event ? Number($event) : null"
          /><button
            :disabled="selectedEntry == null || busy"
            @click="loadEntry"
          >
            {{ t("rasterEditor.text062") }}
          </button></template
        >
        <p>{{ t("rasterEditor.text063") }} {{ label(decal.atlas) }}</p>
        <p>{{ t("rasterEditor.text064") }}</p>
      </template>
      <template v-if="profile === 'lot-mask' || isDecal">
        <button
          :disabled="busy || !isTauri() || !packages.opened.length"
          @click="scan"
        >
          {{
            busy
              ? t("rasterEditor.text065")
              : t("rasterEditor.text066") +
                (isDecal ? t("rasterEditor.text067") : "Lot")
          }}
        </button>
        <label v-if="documents.length">
          {{ t("rasterEditor.text068") }}
          <input
            v-model="documentQuery"
            :placeholder="t('rasterEditor.text061')" /></label
        ><FSelect
          v-if="documents.length"
          v-model="selected"
          :disabled="disabled || busy"
          :options="documentOptions"
        />
        <button :disabled="!selected" @click="select">
          {{ isDecal ? t("rasterEditor.text069") : t("rasterEditor.text070") }}
        </button>
      </template>
    </fieldset>
    <p v-if="error" role="alert">{{ error }}</p>
  </section>
</template>
<style scoped>
.material-panel {
  padding: 16px;
  border-top: 1px solid var(--border);
}
h3 {
  margin: 0 0 12px;
  font-size: 12px;
}
fieldset {
  border: 0;
  padding: 0;
  min-width: 0;
  display: grid;
  gap: 10px;
}
label {
  display: flex;
  gap: 6px;
  align-items: center;
  flex-wrap: wrap;
  font-size: 11px;
  color: var(--muted-foreground);
}
input,
button,
select {
  background: var(--surface);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 5px;
  min-width: 0;
  font-size: 11px;
}
input:not([type="color"], [type="checkbox"]) {
  width: 100%;
}
input[type="color"] {
  width: 32px;
  padding: 2px;
}
.colors,
.color-row {
  display: flex;
  gap: 10px;
}
.color-row label {
  flex: 1;
}
p {
  font-size: 11px;
  color: var(--muted-foreground);
  margin: 0;
  line-height: 1.6;
}
[role="alert"] {
  color: var(--destructive);
}
button {
  cursor: pointer;
}
</style>


