<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { computed, onBeforeUnmount, ref, shallowRef, watch } from "vue";
import { isTauri, tauriApi } from "@/api";
import FSelect from "@/components/ui/FSelect.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FTooltip from "@/components/ui/FTooltip.vue";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { RasterDocument } from "@/lib/raster-editor/document";
import { base64ToRgba } from "@/lib/raster-editor/encoders";
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
import RasterToolbar from "./RasterToolbar.vue";
import RasterParameterTable from "./RasterParameterTable.vue";
import RasterSourcePanel from "./RasterSourcePanel.vue";
import RasterMaterialPanel from "./RasterMaterialPanel.vue";
import RasterPreviewPanel from "./RasterPreviewPanel.vue";
import { useRasterSession } from "./useRasterSession";
import {
  PROFILE_BEHAVIOR,
  layerPaint,
  pickedLayer,
} from "@/lib/raster-editor/profileBehavior";
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
const editorStarted = ref(!!props.initialSchema || !!props.initialProfile);
const packages = useGamePackagesStore();
const canvas = ref<InstanceType<typeof RasterCanvas> | null>(null);
const tool = ref<RasterTool>("brush"),
  size = ref(4),
  color = ref("#ffffff"),
  alpha = ref(255),
  channel = ref<Channel>(PROFILE_BEHAVIOR[profile.value].defaultChannel),
  layer = ref(0),
  tolerance = ref(0);
const parkingLength = ref(13),
  parkingSpacing = ref(6);
const baseLotPixels = shallowRef<Uint8ClampedArray | null>(null);
const lotOverlayPixels = shallowRef<Uint8ClampedArray | null>(null);
const lotOverlayTouched = shallowRef<Uint8Array | null>(null);
const lotLastPixels = shallowRef<Uint8ClampedArray | null>(null);
const baseLotVisible = ref(true);
const baseLotLocked = ref(false);
const error = ref(""),
  status = ref(""),
  saving = ref(false),
  schemaText = ref("");
const locked = computed(() => saving.value || props.busy);
const descriptor = computed(() =>
  RASTER_PROFILES.find((p) => p.id === profile.value)!,
);
const brushColor = computed<Rgba>(() => {
  if (PROFILE_BEHAVIOR[profile.value].layers && channel.value === "rgba")
    return layerPaint(
      profile.value,
      layer.value,
      profile.value === "decal-graffiti" ? alpha.value : 255,
    );
  const rgb = [
    parseInt(color.value.slice(1, 3), 16),
    parseInt(color.value.slice(3, 5), 16),
    parseInt(color.value.slice(5, 7), 16),
  ];
  return [rgb[0], rgb[1], rgb[2], Math.min(255, Math.max(0, alpha.value || 0))];
});
const channelValue = computed({
  get: () => {
    if (channel.value === "a") return alpha.value;
    const offset = { r: 1, g: 3, b: 5, rgba: 1 }[channel.value];
    return parseInt(color.value.slice(offset, offset + 2), 16);
  },
  set: (value: number) => {
    const byte = Math.max(0, Math.min(255, Math.round(value || 0)));
    if (channel.value === "a") alpha.value = byte;
    else color.value = "#" + byte.toString(16).padStart(2, "0").repeat(3);
  },
});
let generation = 0;
onBeforeUnmount(() => {
  generation++;
});
watch(dirty, (value) => emit("dirty", value));
watch(profile, () => {
  channel.value = PROFILE_BEHAVIOR[profile.value].defaultChannel;
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
  editorStarted.value = true;
  if (value !== profile.value) {
    generation++;
    session.changeProfile(value);
  }
}
const profileOptions = computed(() =>
  RASTER_PROFILES.map((item) => ({ value: item.id, label: item.label })),
);
function toggleLotBaseVisible() {
  if (!doc.value || !baseLotPixels.value || !lotOverlayPixels.value) return;
  baseLotVisible.value = !baseLotVisible.value;
  composeLotLayers();
  version.value++;
}
function composeLotLayers() {
  if (!doc.value || !baseLotPixels.value || !lotOverlayPixels.value || !lotOverlayTouched.value) return;
  for (let i = 0, pixel = 0; i < doc.value.pixels.length; i += 4, pixel += 1) {
    const base = baseLotVisible.value ? baseLotPixels.value[i] : 0;
    const overlay = lotOverlayTouched.value[pixel] ? lotOverlayPixels.value[i] : base;
    doc.value.pixels[i] = overlay;
    doc.value.pixels[i + 1] = lotOverlayTouched.value[pixel] ? lotOverlayPixels.value[i + 1] : (baseLotVisible.value ? baseLotPixels.value[i + 1] : 0);
    doc.value.pixels[i + 2] = lotOverlayTouched.value[pixel] ? lotOverlayPixels.value[i + 2] : (baseLotVisible.value ? baseLotPixels.value[i + 2] : 0);
    doc.value.pixels[i + 3] = lotOverlayTouched.value[pixel] ? lotOverlayPixels.value[i + 3] : (baseLotVisible.value ? baseLotPixels.value[i + 3] : 0);
  }
  lotLastPixels.value = doc.value.pixels.slice();
}
function onCanvasChange() {
  if (profile.value === "lot-mask" && baseLotPixels.value && lotOverlayPixels.value && lotOverlayTouched.value && doc.value) {
    if (!baseLotLocked.value) {
      baseLotPixels.value = doc.value.pixels.slice();
      lotOverlayPixels.value.fill(0);
      lotOverlayTouched.value.fill(0);
    } else {
      const previous = lotLastPixels.value ?? baseLotPixels.value;
      for (let i = 0, pixel = 0; i < doc.value.pixels.length; i += 4, pixel += 1) {
        const changed = doc.value.pixels[i] !== previous[i]
          || doc.value.pixels[i + 1] !== previous[i + 1]
          || doc.value.pixels[i + 2] !== previous[i + 2]
          || doc.value.pixels[i + 3] !== previous[i + 3];
        if (changed) {
          lotOverlayPixels.value[i] = doc.value.pixels[i];
          lotOverlayPixels.value[i + 1] = doc.value.pixels[i + 1];
          lotOverlayPixels.value[i + 2] = doc.value.pixels[i + 2];
          lotOverlayPixels.value[i + 3] = doc.value.pixels[i + 3];
          lotOverlayTouched.value[pixel] = 1;
        }
      }
    }
    composeLotLayers();
  }
  changed();
}
function pick(rgba: Rgba) {
  color.value =
    "#" +
    rgba
      .slice(0, 3)
      .map((n) => n.toString(16).padStart(2, "0"))
      .join("");
  alpha.value = rgba[3];
  const picked = pickedLayer(profile.value, rgba);
  if (picked != null) layer.value = picked;
  if (profile.value === "decal-graffiti" && picked != null)
    alpha.value = rgba[picked];
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
    baseLotPixels.value = value.image.pixels.slice();
    lotOverlayPixels.value = new Uint8ClampedArray(value.image.pixels.length);
    lotOverlayTouched.value = new Uint8Array(value.image.width * value.image.height);
    lotLastPixels.value = value.image.pixels.slice();
    baseLotVisible.value = true;
    baseLotLocked.value = true;
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
    const tgi = { ...entry.raster, typeId: entry.raster.typeId || 0x2f4e681c };
    const ids = [
      ...new Set(
        [
          atlas.packageId,
          ...packages.opened.map((p) => p.package.packageId),
        ].filter((v): v is number => v != null),
      ),
    ];
    for (const id of ids) {
      const image = await tauriApi.raster
        .readSource(id, tgi)
        .catch(() => tauriApi.raster.readRgba(id, tgi).catch(() => null));
      if (epoch !== generation) return;
      if (!image?.rgbaBase64) continue;
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
      <div class="profile-control">
        <span class="control-label">{{ t("rasterEditor.text083") }}</span>
        <FSelect
          class="profile-select"
          :model-value="editorStarted ? profile : ''"
          :options="[
            { value: '', label: t('rasterEditor.text170') },
            ...profileOptions,
          ]"
          :disabled="locked"
          @update:model-value="
            (value) => value && changeProfile(value as RasterProfile)
          "
        />
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
          v-else-if="profile === 'lot-mask'"
          class="primary"
          :disabled="!doc || locked || !isTauri()"
          @click="save('lot')"
        >
          {{ t("rasterEditor.text031") }}
        </button><button
          v-else-if="profile !== 'slot0'"
          class="primary"
          :disabled="!doc || locked || !isTauri()"
          @click="save('raster')"
        >
          {{ t("rasterEditor.text019") }}
        </button>
      </div>
    </header>
    <div class="editor-layout" :class="{ 'is-empty': !editorStarted }">
      <aside v-if="editorStarted" class="left-rail">
        <div v-if="profile === 'lot-mask' && doc" class="layer-section">
          <h2>{{ t("rasterEditor.baseLayer") }}</h2>
          <div v-if="baseLotPixels" class="base-layer-row">
            <span class="layer-swatch base-layer-swatch" />
            <span class="base-layer-name">{{ t("rasterEditor.baseLayer") }}</span>
            <FTooltip :text="baseLotVisible ? t('rasterEditor.hideLayer') : t('rasterEditor.showLayer')">
              <template #trigger><button type="button" class="layer-icon-button" :aria-label="baseLotVisible ? t('rasterEditor.hideLayer') : t('rasterEditor.showLayer')" @click="toggleLotBaseVisible"><FIcon :name="baseLotVisible ? 'Eye' : 'EyeOff'" :size="14" /></button></template>
            </FTooltip>
            <FTooltip :text="baseLotLocked ? t('rasterEditor.unlockLayer') : t('rasterEditor.lockLayer')">
              <template #trigger><button type="button" class="layer-icon-button" :aria-label="baseLotLocked ? t('rasterEditor.unlockLayer') : t('rasterEditor.lockLayer')" @click="baseLotLocked = !baseLotLocked"><FIcon :name="baseLotLocked ? 'Lock' : 'LockOpen'" :size="14" /></button></template>
            </FTooltip>
          </div>
          <div v-if="lotOverlayPixels" class="base-layer-row overlay-layer-row">
            <span class="layer-swatch overlay-layer-swatch" />
            <span class="base-layer-name">{{ t("rasterEditor.drawingLayer") }}</span>
          </div>
        </div>
        <div v-if="editorStarted" class="rail-section source-section">
          <RasterSourcePanel
            :image-width="doc?.width"
            :image-height="doc?.height"
            @load-lot="loadLot"
            @load-decal="loadDecal"
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
        </div>
      </aside>
      <main class="workspace" :inert="locked">
        <div v-if="editorStarted" class="document-heading">
          <div>
            <strong>{{ descriptor.label }}</strong>
          </div>
          <span class="dirty-state">{{
            dirty ? t("rasterEditor.text020") : t("rasterEditor.text021")
          }}</span>
        </div>
        <label v-if="editorStarted" class="document-name">
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
          <div class="canvas-stage">
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
              :quantized="profile === 'lot-mask'"
              :channel="channel"
              display-channel="rgba"
              :lot-layer="profile === 'lot-mask' ? layer : null"
              :data-channels="PROFILE_BEHAVIOR[profile].dataChannels"
              :meters-per-pixel="profile === 'lot-mask' ? sampling.metersPerPixel : null"
              @change="onCanvasChange"
              @pick-color="pick"
              @pick-layer="layer = $event; tool = 'brush'"
            />
            <RasterToolbar
              v-model:tool="tool"
              v-model:layer="layer"
              :profile="profile"
              :lot="profile === 'lot-mask'"
              :can-undo="canUndo"
              :can-redo="canRedo"
              @undo="canvas?.undo()"
              @redo="canvas?.redo()"
            />
          </div>
          <div class="editor-guidance">
            <p v-if="profile === 'lot-mask'">{{ t("rasterEditor.lotGuide") }}</p>
            <p>{{ t(`rasterEditor.profileGuide.${profile}`) }}</p>
          </div>
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
        <div v-else class="empty-canvas empty-editor">
          <strong>{{ t("rasterEditor.text170") }}</strong>
        </div>
        <div
          v-if="mode === 'standalone' && doc && isTauri() && profile.startsWith('decal-')"
          class="material-actions"
        >
          <button
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
      <aside v-if="editorStarted" class="inspector">
        <section class="editor-controls">
          <h2>{{ t("rasterEditor.text171") }}</h2>
          <label>{{ t("rasterEditor.text113") }}<input v-model.number="size" type="number" min="1" max="256" /></label>
          <label v-if="tool === 'fill'">{{ t("rasterEditor.text114") }}<input v-model.number="tolerance" type="number" min="0" max="510" /></label>
          <template v-if="tool === 'parking'">
            <label>{{ t("rasterEditor.text115") }}<input v-model.number="parkingLength" type="number" min="1" max="256" /></label>
            <label>{{ t("rasterEditor.text116") }}<input v-model.number="parkingSpacing" type="number" min="1" max="256" /></label>
          </template>
          <label v-if="!mask && channel === 'rgba'">RGB <input v-model="color" type="color" /></label>
          <label v-if="channel === 'rgba' && (!mask || profile === 'decal-graffiti')">{{ t("rasterEditor.channelStrength") }}<input v-model.number="alpha" type="number" min="0" max="255" /></label>
          <label v-if="channel !== 'rgba'">{{ t("rasterEditor.channelValue", { channel: channel.toUpperCase() }) }}<input v-model.number="channelValue" type="number" min="0" max="255" /></label>
          <label>{{ t("rasterEditor.text122") }}<FSelect :model-value="channel" :options="['rgba','r','g','b','a'].map((value) => ({ value, label: profile === 'lot-mask' ? t(`rasterEditor.channels.${value}`) : value.toUpperCase() }))" @update:model-value="channel = $event as Channel" /></label>
        </section>
        <RasterMaterialPanel
          v-if="editorStarted && profile !== 'slot0'"
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
          v-if="editorStarted && doc && profile !== 'slot0'"
          :doc="doc"
          :version="version"
          :profile="profile"
          :colors="lot.colors"
          :decal-colors="decal.colors"
          :decal-aspect-ratio="decal.aspectRatio"
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
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: 0;
  min-width: 0;
}
.editor-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 16px;
  min-height: 54px;
  margin-bottom: 0;
  padding: 7px 16px;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}
.profile-control {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.control-label {
  color: var(--muted-foreground);
  font-size: 11px;
  white-space: nowrap;
}
.profile-select { width: min(260px, 32vw); }
.profile-select :deep(.f-select) { min-height: 34px; }
.context-label {
  color: var(--muted-foreground);
  font-size: 11px;
}
h1 {
  font-size: 20px;
  margin: 5px 0 0;
}
h1 span {
  font-size: 16px;
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
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 208px minmax(0, 1fr) 304px;
  border: 0;
  border-radius: 0;
  background: var(--surface);
  overflow: hidden;
}
.editor-layout.is-empty { grid-template-columns: minmax(0, 1fr); }
.left-rail {
  min-width: 0;
  min-height: 0;
  overflow: auto;
  background: var(--surface-elevated);
}
.rail-section + .rail-section {
  border-top: 1px solid var(--border);
}
.layer-section { padding: 16px 10px; }
.layer-section h2 {
  margin: 0 6px 10px;
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
}
.layer-button {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  margin-top: 4px;
  padding: 7px 8px;
  text-align: start;
  border-color: transparent;
  background: transparent;
}
.layer-button[aria-pressed="true"] {
  color: var(--primary);
  background: var(--accent);
  border-color: var(--border);
}
.layer-swatch {
  flex: none;
  width: 16px;
  height: 16px;
  border: 1px solid var(--border-strong);
  border-radius: 3px;
}
.base-layer-row {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) 24px 24px;
  align-items: center;
  gap: 6px;
  margin-top: 12px;
  padding: 8px;
  border-top: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 11px;
}
.base-layer-swatch { background: var(--surface-hover); }
.overlay-layer-row { border-top: 0; margin-top: 0; }
.overlay-layer-swatch { background: color-mix(in srgb, var(--primary) 38%, var(--surface)); }
.base-layer-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.layer-icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 1px solid transparent;
  background: transparent;
  color: var(--muted-foreground);
}
.layer-icon-button:hover { color: var(--foreground); background: var(--surface-hover); }
.profile-section :deep(.raster-profiles) {
  padding: 14px 10px;
}
.source-section :deep(.source-panel) {
  padding: 14px;
}
.workspace {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  border-inline: 1px solid var(--border);
  background: var(--background);
}
.document-heading {
  flex: none;
  padding: 12px 16px 8px;
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
  padding: 0 16px 8px;
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
.canvas-stage {
  flex: 1;
  min-height: 0;
  position: relative;
  display: flex;
  padding: 0 10px 10px;
}
.editor-canvas {
  flex: 1;
  min-height: 260px;
  height: auto;
  border-radius: var(--radius-sm);
}
.canvas-stage :deep(.raster-toolbar) {
  position: absolute;
  z-index: 5;
  left: 50%;
  bottom: 50px;
  width: fit-content;
  max-width: calc(100% - 44px);
  transform: translateX(-50%);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--surface-elevated) 94%, transparent);
  box-shadow: var(--shadow-lg);
  backdrop-filter: blur(14px);
}
.document-footer {
  display: flex;
  flex-wrap: nowrap;
  gap: 8px;
  min-height: 0;
  height: 1.2em;
  padding: 0 8px;
  border-top: 1px solid var(--border);
  font-size: 9px;
  line-height: 1.2;
  color: var(--muted-foreground);
  align-items: center;
  overflow: hidden;
  white-space: nowrap;
}
.empty-canvas {
  flex: 1;
  min-height: 0;
  display: grid;
  place-content: center;
  padding: 32px;
  text-align: center;
  background: radial-gradient(circle, var(--border) 1px, transparent 1px) 0/20px
    20px;
}
.empty-editor { background: var(--background); }
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
  min-height: 0;
  background: var(--surface-elevated);
  overflow: auto;
}
.editor-controls {
  display: grid;
  gap: 10px;
  padding: 14px;
  border-bottom: 1px solid var(--border);
}
.editor-controls h2 {
  margin: 0 0 2px;
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
}
.editor-controls label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  color: var(--muted-foreground);
  font-size: 11px;
  white-space: nowrap;
}
.editor-controls input {
  min-width: 0;
  width: 116px;
  padding: 5px 7px;
  color: var(--foreground);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
.editor-controls input[type="color"] { width: 34px; padding: 2px; }
.editor-controls :deep(.f-select) { min-height: 30px; font-size: 11px; }
.editor-guidance {
  flex: none;
  margin: 0 10px 8px;
  padding: 8px 12px;
  border: 1px solid color-mix(in srgb, var(--warning) 42%, var(--border));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--warning) 13%, var(--surface));
  color: color-mix(in srgb, var(--warning) 70%, var(--foreground));
}
.editor-guidance p {
  margin: 0;
  font-size: 11px;
  line-height: 1.5;
}
.editor-guidance p + p { margin-top: 3px; }
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
  padding: 8px;
}
@media (max-width: 1100px) {
  .editor-layout {
    grid-template-columns: 190px minmax(0, 1fr);
  }
  .editor-layout.is-empty { grid-template-columns: minmax(0, 1fr); }
  .inspector {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    border-top: 1px solid var(--border);
    max-height: 360px;
  }
}
@media (max-width: 700px) {
  .raster-editor {
    height: auto;
    min-height: calc(100dvh - 56px);
  }
  .editor-layout {
    display: block;
    overflow: visible;
  }
  .left-rail { max-height: none; }
  .workspace {
    border: 0;
    border-block: 1px solid var(--border);
  }
  .inspector {
    display: block;
  }
  .canvas-stage { min-height: 480px; }
  .canvas-stage :deep(.raster-toolbar) {
    left: 50%;
    bottom: 42px;
    max-width: calc(100% - 24px);
  }
}
</style>
