<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { isTauri, tauriApi } from "@/api";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { RasterDocument } from "@/lib/raster-editor/document";
import {
  LOT_LAYERS,
  layerOfPixel,
  base64ToRgba,
} from "@/lib/raster-editor/encoders";
import {
  RASTER_PROFILES,
  type Channel,
  type RasterProfile,
  type RasterSchema,
  type RasterSource,
  type RasterTool,
} from "@/lib/raster-editor/schema";
import type { Rgba } from "@/lib/raster-editor/tools";
import type { DecalEntryMeta, PropertyDocumentSummary } from "@/api/tauri";
import RasterCanvas from "./RasterCanvas.vue";
import RasterProfilePicker from "./RasterProfilePicker.vue";
import RasterToolbar from "./RasterToolbar.vue";
import RasterParameterTable from "./RasterParameterTable.vue";
import RasterSourcePanel from "./RasterSourcePanel.vue";
import RasterMaterialPanel from "./RasterMaterialPanel.vue";
import RasterPreviewPanel from "./RasterPreviewPanel.vue";
import { useRasterSession } from "./useRasterSession";
import {
  downloadJson,
  downloadRasterAsset,
  loadLotCopy,
  saveRasterCopy,
} from "./rasterIO";
const props = withDefaults(
  defineProps<{
    mode?: "standalone" | "flow";
    initialSchema?: RasterSchema;
    initialProfile?: RasterProfile;
    lockedProfile?: boolean;
    busy?: boolean;
  }>(),
  { mode: "standalone" },
);
const emit = defineEmits<{
  schema: [value: RasterSchema];
  dirty: [value: boolean];
}>();
const session = useRasterSession(props.initialSchema);
const {
  profile,
  name,
  doc,
  history,
  parameters,
  source,
  dirty,
  version,
  sampling,
  lot,
  decal,
  mask,
  ready,
  canUndo,
  canRedo,
  changed,
} = session;
if (!props.initialSchema && props.initialProfile)
  profile.value = props.initialProfile;
const packages = useGamePackagesStore();
const canvas = ref<InstanceType<typeof RasterCanvas> | null>(null);
const tool = ref<RasterTool>("brush"),
  size = ref(4),
  color = ref("#ffffff"),
  alpha = ref(255),
  channel = ref<Channel>("rgba"),
  layer = ref(0),
  tolerance = ref(0),
  quantized = ref(false);
const parkingLength = ref(13),
  parkingSpacing = ref(6);
const error = ref(""),
  status = ref(""),
  saving = ref(false),
  schemaText = ref("");
const locked = computed(() => saving.value || props.busy);
const descriptor = computed(() =>
  RASTER_PROFILES.find((p) => p.id === profile.value)!,
);
const brushColor = computed<Rgba>(() => {
  if (mask.value && channel.value === "rgba")
    return [...LOT_LAYERS[layer.value].paint] as Rgba;
  const rgb = [
    parseInt(color.value.slice(1, 3), 16),
    parseInt(color.value.slice(3, 5), 16),
    parseInt(color.value.slice(5, 7), 16),
  ];
  return [rgb[0], rgb[1], rgb[2], Math.min(255, Math.max(0, alpha.value || 0))];
});
let generation = 0;
onBeforeUnmount(() => {
  generation++;
});
watch(dirty, (value) => emit("dirty", value));
watch(profile, () => {
  quantized.value = false;
  channel.value = "rgba";
  tool.value = "brush";
  schemaText.value = "";
});
watch(version, () => {
  schemaText.value = "";
});
function replace(action: () => void) {
  if (dirty.value && ready.value && !window.confirm(t("rasterEditor.text001")))
    return;
  generation++;
  error.value = "";
  status.value = "";
  action();
}
function image(
  value: { width: number; height: number; pixels: Uint8ClampedArray },
  origin: RasterSource,
) {
  replace(() =>
    session.install(
      new RasterDocument(value.width, value.height, value.pixels),
      origin,
      t("rasterEditor.text002", { p0: origin.name }),
    ),
  );
}
function restore(value: RasterSchema) {
  if (props.lockedProfile && value.profile !== profile.value) {
    error.value = t("rasterEditor.text003");
    return;
  }
  // packageId is only valid in the originating process. Imported documents must rebind native targets.
  for (const origin of [value.source, value.lot.target, value.decal.atlas])
    if (origin) delete origin.packageId;
  replace(() => {
    session.load(value);
    if (props.mode === "flow") changed();
  });
}
function changeProfile(value: RasterProfile) {
  if (value !== profile.value) {
    generation++;
    session.changeProfile(value);
  }
}
function pick(rgba: Rgba) {
  color.value =
    "#" +
    rgba
      .slice(0, 3)
      .map((n) => n.toString(16).padStart(2, "0"))
      .join("");
  alpha.value = rgba[3];
  const picked = layerOfPixel(rgba);
  if (picked != null) layer.value = picked;
}
function output(download = false) {
  error.value = "";
  try {
    const schema = session.snapshot();
    schemaText.value = JSON.stringify(schema, null, 2);
    if (download) downloadJson(schema, `${name.value}.raster.json`);
    else emit("schema", schema);
    status.value = download
      ? t("rasterEditor.text004")
      : t("rasterEditor.text005");
  } catch (e) {
    error.value = String(e);
  }
}
function exportAsset() {
  try {
    downloadRasterAsset(session.snapshot());
    error.value = "";
  } catch (e) {
    error.value = String(e);
  }
}
async function save(kind: "raster" | "lot" | "decal") {
  if (locked.value) return;
  saving.value = true;
  error.value = "";
  status.value = "";
  try {
    const path = await saveRasterCopy(session.snapshot(), kind);
    if (path) {
      dirty.value = false;
      status.value = t("rasterEditor.text006", { p0: path });
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    saving.value = false;
  }
}
async function loadLot(
  summary: PropertyDocumentSummary & { packageId: number },
) {
  if (dirty.value && !window.confirm(t("rasterEditor.text007"))) return;
  const epoch = ++generation;
  saving.value = true;
  error.value = "";
  try {
    const value = await loadLotCopy(
      summary,
      packages.opened.map((p) => p.package.packageId),
    );
    if (epoch !== generation) return;
    session.changeProfile("lot-mask");
    session.install(
      new RasterDocument(
        value.image.width,
        value.image.height,
        value.image.pixels,
      ),
      value.source,
      value.source.name,
    );
    lot.value = { colors: value.material.colors, target: value.target };
  } catch (e) {
    if (epoch === generation) error.value = String(e);
  } finally {
    saving.value = false;
  }
}
async function loadDecal(entry: DecalEntryMeta, atlas: RasterSource) {
  if (!entry.raster || !entry.id) {
    error.value = t("rasterEditor.text008");
    return;
  }
  if (dirty.value && !window.confirm(t("rasterEditor.text009"))) return;
  const epoch = ++generation;
  saving.value = true;
  error.value = "";
  try {
    const tgi = { ...entry.raster, typeId: 0x2f4e681c };
    const ids = [
      ...new Set(
        [
          atlas.packageId,
          ...packages.opened.map((p) => p.package.packageId),
        ].filter((v): v is number => v != null),
      ),
    ];
    for (const id of ids) {
      const image = await tauriApi.raster.readRgba(id, tgi).catch(() => null);
      if (epoch !== generation) return;
      if (!image?.decodable || !image.rgbaBase64) continue;
      session.install(
        new RasterDocument(
          image.width,
          image.height,
          base64ToRgba(image.rgbaBase64),
        ),
        {
          name: `Decal 0x${entry.id.instance.toString(16)}`,
          packageId: id,
          tgi,
        },
        `Decal 0x${entry.id.instance.toString(16)}`,
      );
      decal.value = {
        ...decal.value,
        atlas,
        entryId: `0x${entry.id.instance.toString(16)}`,
        entryTgi: { ...entry.id },
        replaceInstance: `0x${entry.id.instance.toString(16)}`,
        aspectRatio: entry.aspectRatio ?? image.width / image.height,
        colors: entry.colorsRgba8?.map(
          (c) =>
            "#" +
            c
              .slice(0, 3)
              .map((v) => v.toString(16).padStart(2, "0"))
              .join(""),
        ) ?? ["#ffffff", "#ffffff", "#ffffff", "#ffffff"],
      };
      return;
    }
    throw new Error(t("rasterEditor.text010"));
  } catch (e) {
    if (epoch === generation) error.value = String(e);
  } finally {
    saving.value = false;
  }
}
</script>
<template>
  <section
    class="raster-editor"
    :class="{ embedded: mode === 'flow' }"
    :aria-label="t('rasterEditor.text011')"
  >
    <header class="editor-heading">
      <div>
        <span class="context-label">{{
          mode === "flow"
            ? t("rasterEditor.text012")
            : t("rasterEditor.text013")
        }}</span>
        <h1>
          Raster <span> {{ t("rasterEditor.text014") }} </span>
        </h1>
      </div>
      <div class="actions">
        <button :disabled="!ready || locked" @click="exportAsset">
          {{
            profile === "slot0"
              ? t("rasterEditor.text015")
              : t("rasterEditor.text016")
          }}</button
        ><button :disabled="!ready || locked" @click="output(true)">
          {{ t("rasterEditor.text017") }}</button
        ><button
          v-if="mode === 'flow'"
          class="primary"
          :disabled="!ready || locked"
          @click="output()"
        >
          {{ t("rasterEditor.text018") }}</button
        ><button
          v-else-if="profile !== 'slot0'"
          class="primary"
          :disabled="!doc || locked || !isTauri()"
          @click="save('raster')"
        >
          {{ t("rasterEditor.text019") }}
        </button>
      </div>
    </header>
    <div class="editor-layout">
      <RasterProfilePicker
        :model-value="profile"
        :locked="lockedProfile || locked"
        @update:model-value="changeProfile"
      />
      <main class="workspace" :inert="locked">
        <div class="document-heading">
          <div>
            <strong>{{ descriptor.label }}</strong>
            <p>{{ descriptor.hint }}</p>
          </div>
          <span class="dirty-state">{{
            dirty ? t("rasterEditor.text020") : t("rasterEditor.text021")
          }}</span>
        </div>
        <label class="document-name">
          {{ t("rasterEditor.text022") }}
          <input v-model="name" :disabled="locked" @input="changed"
        /></label>
        <RasterParameterTable
          v-if="profile === 'slot0'"
          :model-value="parameters"
          @update:model-value="
            parameters = $event;
            changed();
          "
        />
        <template v-else-if="doc && history">
          <RasterToolbar
            v-model:tool="tool"
            v-model:size="size"
            v-model:color="color"
            v-model:alpha="alpha"
            v-model:channel="channel"
            v-model:layer="layer"
            v-model:quantized="quantized"
            v-model:tolerance="tolerance"
            v-model:parking-length="parkingLength"
            v-model:parking-spacing="parkingSpacing"
            :mask="mask"
            :lot="profile === 'lot-mask'"
            :can-undo="canUndo"
            :can-redo="canRedo"
            @undo="canvas?.undo()"
            @redo="canvas?.redo()"
          />
          <RasterCanvas
            :key="`${profile}:${doc.width}:${doc.height}`"
            ref="canvas"
            class="editor-canvas"
            :doc="doc"
            :history="history"
            :tool="tool"
            :color="brushColor"
            :brush-size="Math.max(1, Math.min(256, size || 1))"
            :parking-length="Math.max(1, Math.min(256, parkingLength || 1))"
            :parking-spacing="Math.max(1, Math.min(256, parkingSpacing || 1))"
            :fill-tolerance="Math.max(0, Math.min(510, tolerance || 0))"
            :quantized="mask && quantized && channel === 'rgba'"
            :channel="channel"
            :meters-per-pixel="
              profile === 'lot-mask' ? sampling.metersPerPixel : null
            "
            @change="changed"
            @pick-color="pick"
          />
          <footer class="document-footer">
            <code>{{ doc.width }} × {{ doc.height }} · RGBA8</code
            ><span>{{
              source?.tgi
                ? t("rasterEditor.text023") + source.tgi.instance.toString(16)
                : t("rasterEditor.text024")
            }}</span
            ><span>{{
              channel === "rgba"
                ? t("rasterEditor.text025")
                : t("rasterEditor.text026") +
                  channel.toUpperCase() +
                  t("rasterEditor.text027")
            }}</span>
          </footer>
        </template>
        <div v-else class="empty-canvas">
          <strong> {{ t("rasterEditor.text028") }} </strong>
          <p>{{ t("rasterEditor.text029") }}</p>
          <p>{{ t("rasterEditor.text030") }}</p>
        </div>
        <div
          v-if="mode === 'standalone' && doc && isTauri()"
          class="material-actions"
        >
          <button
            v-if="profile === 'lot-mask'"
            :disabled="locked"
            @click="save('lot')"
          >
            {{ t("rasterEditor.text031") }}</button
          ><button
            v-if="profile.startsWith('decal-')"
            :disabled="locked || !decal.atlas"
            @click="save('decal')"
          >
            {{
              decal.replaceInstance
                ? t("rasterEditor.text032")
                : t("rasterEditor.text033")
            }}
          </button>
        </div>
        <p v-if="error" class="message error" role="alert">{{ error }}</p>
        <p v-if="status" class="message" role="status">{{ status }}</p>
        <details v-if="schemaText" class="schema-output">
          <summary>{{ t("rasterEditor.text034") }}</summary>
          <p v-if="schemaText.length > 8000">{{ t("rasterEditor.text035") }}</p>
          <pre>{{ schemaText.slice(0, 8000) }}</pre>
        </details>
      </main>
      <aside class="inspector">
        <RasterSourcePanel
          :profile="profile"
          :disabled="locked"
          @create="(w, h) => replace(() => session.create(w, h))"
          @image="image"
          @schema="restore"
          @parameters="
            (value) =>
              replace(() => {
                parameters = value;
                changed();
              })
          "
        />
        <RasterMaterialPanel
          v-if="profile !== 'slot0'"
          v-model:lot="lot"
          v-model:decal="decal"
          v-model:sampling="sampling"
          :profile="profile"
          :disabled="locked"
          @load-lot="loadLot"
          @load-decal="loadDecal"
          @changed="changed"
        />
        <RasterPreviewPanel
          v-if="doc && profile !== 'slot0'"
          :doc="doc"
          :version="version"
          :channel="channel"
          :profile="profile"
          :colors="lot.colors"
          :target="lot.target"
          :meters-per-pixel="sampling.metersPerPixel"
        />
      </aside>
    </div>
  </section>
</template>
<style scoped>
.raster-editor {
  color: var(--foreground);
  padding: 24px;
  min-width: 0;
}
.editor-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 16px;
  margin-bottom: 22px;
}
.context-label {
  color: var(--muted-foreground);
  font-size: 11px;
}
h1 {
  font-size: 26px;
  margin: 5px 0 0;
  letter-spacing: -0.7px;
}
h1 span {
  font-size: 20px;
  font-weight: 500;
  margin-left: 6px;
}
.actions {
  display: flex;
  gap: 8px;
}
button {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 5px;
  padding: 8px 12px;
  color: var(--foreground);
  font-size: 12px;
  cursor: pointer;
}
button.primary {
  background: var(--primary);
  color: var(--primary-foreground, white);
  border-color: var(--primary);
}
button:disabled {
  opacity: 0.45;
  cursor: default;
}
.editor-layout {
  display: grid;
  grid-template-columns: 160px minmax(320px, 1fr) 290px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface);
  overflow: hidden;
}
.workspace {
  min-width: 0;
  border-inline: 1px solid var(--border);
}
.document-heading {
  padding: 16px;
  display: flex;
  gap: 10px;
  justify-content: space-between;
}
.document-heading strong {
  font-size: 14px;
}
.document-heading p {
  font-size: 11px;
  color: var(--muted-foreground);
  line-height: 1.6;
  margin: 5px 0 0;
}
.dirty-state {
  color: var(--muted-foreground);
  font-size: 10px;
  white-space: nowrap;
}
.document-name {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 16px 12px;
  color: var(--muted-foreground);
  font-size: 11px;
}
.document-name input {
  min-width: 0;
  flex: 1;
  background: transparent;
  border: 0;
  border-bottom: 1px solid var(--border);
  color: var(--foreground);
  padding: 6px;
  font-size: 12px;
}
.editor-canvas {
  height: min(58vh, 640px);
  min-height: 360px;
}
.document-footer {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  padding: 12px;
  border-top: 1px solid var(--border);
  font-size: 10px;
  color: var(--muted-foreground);
}
.empty-canvas {
  min-height: 420px;
  display: grid;
  place-content: center;
  padding: 32px;
  text-align: center;
  background: radial-gradient(circle, var(--border) 1px, transparent 1px) 0/20px
    20px;
}
.empty-canvas strong {
  font-size: 18px;
}
.empty-canvas p {
  font-size: 12px;
  color: var(--muted-foreground);
  line-height: 1.8;
}
.inspector {
  min-width: 0;
  max-height: 85vh;
  overflow: auto;
}
.material-actions,
.message {
  padding: 12px 16px;
  font-size: 12px;
  overflow-wrap: anywhere;
}
.error {
  color: var(--destructive);
}
.schema-output {
  padding: 16px;
  font-size: 12px;
}
pre {
  overflow: auto;
  max-height: 220px;
  font-size: 11px;
}
:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}
.embedded {
  padding: 12px;
}
@media (max-width: 1100px) {
  .editor-layout {
    grid-template-columns: 140px minmax(300px, 1fr);
  }
  .inspector {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    border-top: 1px solid var(--border);
    max-height: none;
  }
}
@media (max-width: 700px) {
  .raster-editor {
    padding: 12px;
  }
  .editor-layout {
    display: block;
  }
  .workspace {
    border: 0;
    border-block: 1px solid var(--border);
  }
  .inspector {
    display: block;
  }
}
</style>
