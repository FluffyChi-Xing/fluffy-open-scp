<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { computed, onBeforeUnmount, ref, shallowRef, useId, watch } from "vue";
import { isTauri, tauriApi } from "@/api";
import FSelect, { type SelectOption } from "@/components/ui/FSelect.vue";
import { Switch } from "@/components/ui/switch";
import FIcon from "@/components/extensions/FIcon.vue";
import FTooltip from "@/components/ui/FTooltip.vue";
import { provide } from "vue";
import EditorPackagesBar from "@/components/EditorPackagesBar.vue";
import { EDITOR_PACKAGES, useEditorPackages } from "@/composables/useEditorPackages";
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
import LotEdgeControls from "./LotEdgeControls.vue";
import RasterPreviewPanel from "./RasterPreviewPanel.vue";
import { reverseMaskRows } from "@/lib/lot-ground/maskRows";
import RasterDecalPreview from "./RasterDecalPreview.vue";
import DecalTypeTabs from "./sign/DecalTypeTabs.vue";
import RasterSignControls from "./sign/RasterSignControls.vue";
import GraffitiProjectionPreview from "./sign/GraffitiProjectionPreview.vue";
import {
  createSignPaintPreview,
  createSignLettering,
  defaultSignSettings,
  signDisplayPixels,
  signPalette,
  type SignSettings,
} from "@/lib/raster-editor/signPaint";
import { graffitiDisplayPixels, graffitiPalette } from "@/lib/raster-editor/graffitiPaint";
import {
  canPaintDecalLayer, defaultDecalLayers, stampDecalLettering, visibleDecalPixels,
  type DecalLayerState,
} from "@/lib/raster-editor/decalLayers";
import { useRasterSession } from "./useRasterSession";
import LotGroundSize from "@/components/lot-ground/LotGroundSize.vue";
import LotMaterialPalette from "@/components/lot-ground/LotMaterialPalette.vue";
import { useLotGround } from "@/components/lot-ground/useLotGround";
import { maskSizeForLot, resampleNearest } from "@/lib/lot-ground/ground";
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
    /** flow 模式主按钮文案（缺省"应用到节点"）。 */
    applyLabel?: string;
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
if (!props.initialSchema && props.mode === "standalone" && isDecalStudioProfile(profile.value))
  decal.value = { ...decal.value, colors: [...(profile.value === "decal-sign" ? signPalette : graffitiPalette)] };
const editorStarted = ref(!!props.initialSchema || !!props.initialProfile);
const packages = useEditorPackages("assets");
provide(EDITOR_PACKAGES, packages);
const canvas = ref<InstanceType<typeof RasterCanvas> | null>(null);
const tool = ref<RasterTool>("brush"),
  size = ref(4),
  color = ref("#ffffff"),
  alpha = ref(255),
  channel = ref<Channel>(PROFILE_BEHAVIOR[profile.value].defaultChannel),
  layer = ref(0),
  tolerance = ref(0);
const circleRadius = ref(16);
const fillThroughLocked = ref(false);
const isCircleTool = computed(() => tool.value === "circle" || tool.value === "circle-fill");
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
function isDecalStudioProfile(value: RasterProfile): value is "decal-sign" | "decal-graffiti" {
  return value === "decal-sign" || value === "decal-graffiti";
}
function isPanelProfile(value: RasterProfile) {
  return value === "lot-mask" || isDecalStudioProfile(value);
}
const selectableProfiles = computed(() =>
  RASTER_PROFILES.filter((item) => {
    if (props.lockedProfile) return item.id === profile.value;
    if (props.mode === "standalone") return isPanelProfile(item.id);
    return item.id !== "lot-texture";
  }),
);
const profileOptions = computed(() => {
  const options = selectableProfiles.value.map(
    ({ id, label }): SelectOption => ({ value: id, label }),
  );
  if (!editorStarted.value)
    options.unshift({ value: "", label: t("rasterEditor.text170") });
  return options;
});
const isLot = computed(() => profile.value === "lot-mask");
const decalStudioProfile = computed(() => props.mode === "standalone" && isDecalStudioProfile(profile.value) ? profile.value : null);
const isDecalStudio = computed(() => decalStudioProfile.value !== null);
const isSign = computed(() => decalStudioProfile.value === "decal-sign");
const isGraffiti = computed(() => decalStudioProfile.value === "decal-graffiti");
const sprayStrength = ref(255);
const signId = useId();
const signInspectorOpen = ref(true);
const signSettings = computed(() => decal.value.sign ?? defaultSignSettings);
const signAnimated = computed(() => isSign.value && signSettings.value.effect === "marquee");
const decalLayers = computed(() => decal.value.layers ?? defaultDecalLayers);
const previewDoc = computed(() => {
  const image = doc.value;
  if (!image || decalLayers.value.every(({ visible }) => visible)) return image;
  void version.value;
  return new RasterDocument(image.width, image.height, visibleDecalPixels(image.pixels, decalLayers.value));
});
const signPreview = computed(() => {
  if (!isDecalStudio.value || !doc.value) return null;
  const layers = decalLayers.value;
  const display = isGraffiti.value ? graffitiDisplayPixels : signDisplayPixels;
  return createSignPaintPreview(decal.value.colors, (pixels, colors) =>
    display(visibleDecalPixels(pixels, layers), colors));
});
const signSizes = [
  { name: "wide", width: 256, height: 128 },
  { name: "tall", width: 128, height: 256 },
  { name: "square", width: 128, height: 128 },
] as const;
const graffitiSizes = [
  { name: "wide", width: 128, height: 64 },
  { name: "tall", width: 64, height: 128 },
  { name: "square", width: 64, height: 64 },
] as const;
const decalSizes = computed(() => isGraffiti.value ? graffitiSizes : signSizes);
function setSignSettings(sign: SignSettings) {
  decal.value = { ...decal.value, sign };
  changed();
}
function setSignColor(index: number, color: string) {
  if (decalLayers.value[index].locked) return;
  decal.value = { ...decal.value, colors: decal.value.colors.map((current, i) => i === index ? color : current) };
  changed();
}
function updateDecalLayer(index: number, value: DecalLayerState) {
  canvas.value?.cancelDraft();
  decal.value.layers = decalLayers.value.map((current, i) => i === index ? value : current);
  changed();
}
function createBlank(width: number, height: number) {
  replace(() => session.create(width, height));
}
function addSignLettering(text: string) {
  const image = doc.value;
  if (!image || !canvas.value || !text || !canPaintDecalLayer(decalLayers.value[layer.value])) return;
  const glyphs = createSignLettering(image.width, image.height, text);
  if (!glyphs) return;
  canvas.value.applyEdit((_pixels, before) =>
    stampDecalLettering(image, before, glyphs, decalLayers.value, layer.value, brushColor.value));
}
const ground = useLotGround({
  enabled: isLot,
  lot,
  doc,
  metersPerPixel: computed(() => sampling.value.metersPerPixel),
  changed,
  applyEdit: (mutate) => canvas.value?.applyEdit(mutate),
  confirm: () => window.confirm(t("rasterEditor.ground.removeConfirm")),
});
const groundSlots = ground.slots;
const groundActive = ground.activeSlot;
const groundAtlas = ground.atlas;
const brushColor = computed<Rgba>(() => {
  if (isLot.value) return ground.brush.value;
  if (PROFILE_BEHAVIOR[profile.value].layers && channel.value === "rgba")
    return layerPaint(
      profile.value,
      layer.value,
      isGraffiti.value ? sprayStrength.value : profile.value === "decal-graffiti" ? alpha.value : 255,
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
    return false;
  generation++;
  error.value = "";
  status.value = "";
  action();
  return true;
}
function resetLotLayers() {
  baseLotPixels.value = null;
  lotOverlayPixels.value = null;
  lotOverlayTouched.value = null;
  lotLastPixels.value = null;
}
function applyLotSize(size: [number, number]) {
  const next = maskSizeForLot(size, sampling.value.metersPerPixel);
  const image = doc.value;
  if (!image) {
    if (replace(() => session.create(next.width, next.height))) {
      lot.value = { ...lot.value, size };
      resetLotLayers();
    }
    return;
  }
  if (image.width !== next.width || image.height !== next.height) {
    if (!window.confirm(t("rasterEditor.ground.resizeConfirm"))) return;
    const pixels = resampleNearest(image.pixels, image.width, image.height, next.width, next.height);
    session.install(new RasterDocument(next.width, next.height, pixels), source.value, name.value);
    resetLotLayers();
  }
  lot.value = { ...lot.value, size };
  changed();
}
function onPickLayer(picked: number) {
  if (isLot.value) ground.pickLayer(picked);
  else layer.value = picked;
  tool.value = "brush";
}
function image(
  value: { width: number; height: number; pixels: Uint8ClampedArray },
  origin: RasterSource,
) {
  replace(() =>
    session.install(
      new RasterDocument(
        value.width,
        value.height,
        isLot.value ? reverseMaskRows(value.pixels, value.width, value.height) : value.pixels,
      ),
      origin,
      t("rasterEditor.text002", { p0: origin.name }),
    ),
  );
}
function restore(value: RasterSchema) {
  if (props.mode === "standalone" && !isPanelProfile(value.profile)) {
    error.value = t("rasterEditor.unsupportedPanelProfile");
    return;
  }
  if (props.lockedProfile && value.profile !== profile.value) {
    error.value = t("rasterEditor.text003");
    return;
  }
  // packageId is only valid in the originating process. Imported documents must rebind native targets.
  for (const origin of [value.source, value.lot.target, value.decal.atlas])
    if (origin) delete origin.packageId;
  replace(() => {
    if (value.profile !== profile.value) resetLotLayers();
    session.load(value);
    if (props.mode === "flow") changed();
  });
}
function changeProfile(value: string) {
  const next = selectableProfiles.value.find((item) => item.id === value);
  if (!next) return;
  if (next.id !== profile.value && props.mode === "standalone") {
    if (!replace(() => {
      doc.value = null;
      history.value = null;
      source.value = null;
      resetLotLayers();
      delete decal.value.sign;
      if (isDecalStudioProfile(next.id))
        decal.value = { ...decal.value, colors: [...(next.id === "decal-sign" ? signPalette : graffitiPalette)] };
      session.changeProfile(next.id);
      dirty.value = false;
    })) return;
  } else if (next.id !== profile.value) {
    generation++;
    session.changeProfile(next.id);
  }
  editorStarted.value = true;
}
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
  if (isLot.value && tool.value !== "eraser") ground.ensureActiveUsed();
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
  if (isLot.value) {
    if (picked != null) ground.pickLayer(picked);
    return;
  }
  if (picked != null) layer.value = picked;
  if (profile.value === "decal-graffiti" && picked != null) {
    alpha.value = rgba[picked];
    if (isGraffiti.value) sprayStrength.value = rgba[picked];
  }
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
    error.value = typeof e === "object" && e && "code" in e && e.code === "decal_material_mismatch"
      ? t("rasterEditor.graffiti.dictionaryMismatch")
      : String(e);
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
    lot.value = { ...value.material, target: value.target };
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
defineExpose({
  /** 当前文档快照；尚无文档时返回 null。 */
  snapshot: (): RasterSchema | null => (ready.value ? session.snapshot() : null),
});
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
        entryId: isGraffiti.value ? "" : `0x${entry.id.instance.toString(16)}`,
        entryTgi: isGraffiti.value ? undefined : { ...entry.id },
        replaceInstance: isGraffiti.value ? "" : `0x${entry.id.instance.toString(16)}`,
        aspectRatio: entry.aspectRatio ?? image.width / image.height,
        sign: undefined,
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
    <EditorPackagesBar :scope="packages" />
    <header class="editor-heading">
      <div class="profile-control">
        <span class="control-label">{{ t("rasterEditor.text083") }}</span>
        <FSelect
          class="profile-select"
          :model-value="editorStarted ? profile : ''"
          :options="profileOptions"
          :disabled="locked || lockedProfile"
          @update:model-value="changeProfile"
        />
      </div>
      <div class="actions">
        <button :disabled="!ready || locked" @click="exportAsset">
          {{
            profile === "slot0"
              ? t("rasterEditor.text015")
              : isDecalStudio ? t("rasterEditor.sign.exportMask") : t("rasterEditor.text016")
          }}</button
        ><button :disabled="!ready || locked" @click="output(true)">
          {{ t("rasterEditor.text017") }}</button
        ><button
          v-if="mode === 'flow'"
          class="primary"
          :disabled="!ready || locked"
          @click="output()"
        >
          {{ applyLabel ?? t("rasterEditor.text018") }}</button
        ><button
          v-else-if="profile === 'lot-mask'"
          class="primary"
          :disabled="!doc || locked || !isTauri()"
          @click="save('lot')"
        >
          {{ t("rasterEditor.text031") }}
        </button><button
          v-else-if="isGraffiti"
          class="primary"
          :disabled="!doc || locked || !isTauri() || decal.atlas?.packageId == null"
          @click="save('decal')"
        >
          {{ decal.replaceInstance ? t("rasterEditor.text032") : t("rasterEditor.text033") }}
        </button><button
          v-else-if="profile !== 'slot0' && !signAnimated"
          class="primary"
          :disabled="!doc || locked || !isTauri()"
          @click="save('raster')"
        >
          {{ t("rasterEditor.text019") }}
        </button>
        <button
          v-if="editorStarted && isDecalStudio"
          type="button"
          class="sign-properties-toggle"
          :aria-controls="`${signId}-inspector`"
          :aria-expanded="signInspectorOpen"
          @click="signInspectorOpen = !signInspectorOpen"
        >
          <FIcon name="SlidersHorizontal" :size="15" />
          {{ t("rasterEditor.sign.inspector") }}
        </button>
      </div>
    </header>
    <div class="editor-layout" :class="{ 'is-empty': !editorStarted, 'sign-layout': isDecalStudio, 'sign-inspector-closed': isDecalStudio && !signInspectorOpen }">
      <aside v-if="editorStarted && !isDecalStudio" class="left-rail">
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
        <div class="rail-section source-section">
          <RasterSourcePanel
            :image-width="doc?.width"
            :image-height="doc?.height"
            @load-lot="loadLot"
            @load-decal="loadDecal"
            :profile="profile"
            :disabled="locked"
            @create="createBlank"
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
      <main
        class="workspace"
        :inert="locked"
        :id="isDecalStudio ? `${signId}-panel` : undefined"
        :role="isDecalStudio ? 'tabpanel' : undefined"
        :aria-labelledby="isDecalStudio ? `${signId}-tab-${profile}` : undefined"
      >
        <div v-if="editorStarted && !isDecalStudio" class="document-heading">
          <div>
            <strong>{{ descriptor.label }}</strong>
          </div>
          <span class="dirty-state">{{
            dirty ? t("rasterEditor.text020") : t("rasterEditor.text021")
          }}</span>
        </div>
        <label v-if="editorStarted && !isDecalStudio" class="document-name">
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
            <RasterToolbar
              v-model:tool="tool"
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
              :circle-radius="Math.max(1, Math.min(4096, Math.round(circleRadius || 16)))"
              :parking-length="Math.max(1, Math.min(256, parkingLength || 1))"
              :parking-spacing="Math.max(1, Math.min(256, parkingSpacing || 1))"
              :fill-tolerance="Math.max(0, Math.min(510, tolerance || 0))"
              :fill-through-locked="fillThroughLocked"
              :quantized="profile === 'lot-mask'"
              :north-up="isLot"
              :channel="channel"
              display-channel="rgba"
              :lot-layer="isLot ? ground.canvasLayer.value : null"
              :lot-border-widths="lot.borderWidths"
              :data-channels="PROFILE_BEHAVIOR[profile].dataChannels"
              :meters-per-pixel="isLot ? sampling.metersPerPixel : null"
              :paint-preview="isDecalStudio ? signPreview : ground.paintPreview.value"
              :backdrop="ground.backdrop.value"
              :erase-all-channels="isLot || isDecalStudio"
              :center-on-resize="isDecalStudio"
              :decal-layers="isDecalStudio ? decalLayers : undefined"
              :decal-active-layer="isDecalStudio ? layer : undefined"
              @change="onCanvasChange"
              @pick-color="pick"
              @pick-layer="onPickLayer"
            />
          </div>
          <div class="editor-guidance">
            <p v-if="profile === 'lot-mask'">{{ t("rasterEditor.lotGuide") }}</p>
            <p v-if="isCircleTool">{{ t("rasterEditor.circleHint") }}</p>
            <p v-else-if="isDecalStudio && tool === 'fill'">{{ t("rasterEditor.decalLayers.fillHint") }}</p>
            <p v-else>{{ isDecalStudio ? t(isGraffiti ? "rasterEditor.graffiti.paintHint" : "rasterEditor.sign.paintHint") : t(`rasterEditor.profileGuide.${profile}`) }}</p>
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
        <div v-else class="empty-canvas empty-editor" :class="{ 'sign-empty': isDecalStudio }">
          <div v-if="isDecalStudio" class="sign-start">
            <FIcon name="Brush" :size="22" aria-label="" />
            <strong>{{ t(isGraffiti ? "rasterEditor.graffiti.start" : "rasterEditor.sign.start") }}</strong>
            <p>{{ t(isGraffiti ? "rasterEditor.graffiti.startHint" : "rasterEditor.sign.startHint") }}</p>
            <button type="button" :disabled="locked" @click="createBlank(decalSizes[0].width, decalSizes[0].height)">
              {{ t("rasterEditor.sign.create") }}
            </button>
          </div>
          <strong v-else>{{ t("rasterEditor.text170") }}</strong>
        </div>
        <div
          v-if="mode === 'standalone' && doc && isTauri() && profile.startsWith('decal-') && !isGraffiti && !signAnimated"
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
      <aside
        v-if="editorStarted"
        v-show="!isDecalStudio || signInspectorOpen"
        :id="isDecalStudio ? `${signId}-inspector` : undefined"
        :aria-label="isDecalStudio ? t('rasterEditor.sign.inspector') : undefined"
        :class="{ 'sign-inspector': isDecalStudio }"
        class="inspector"
      >
        <details v-if="isDecalStudio" class="sign-document-settings">
          <summary class="sign-disclosure">
            {{ t("rasterEditor.sign.documentSettings") }}
            <small>{{ dirty ? t("rasterEditor.text020") : t("rasterEditor.text021") }}</small>
          </summary>
          <div class="sign-document-fields">
            <label class="sign-name-field">
              {{ t("rasterEditor.text022") }}
              <input v-model="name" class="sign-control" :disabled="locked" @input="changed" />
            </label>
            <DecalTypeTabs
              v-if="decalStudioProfile" :tab-id="`${signId}-tab`" :panel-id="`${signId}-panel`"
              :profile="decalStudioProfile" :disabled="!!(locked || lockedProfile)" @select="changeProfile"
            />
          </div>
        </details>
        <details v-if="isDecalStudio" :open="!doc" class="sign-canvas-settings">
          <summary class="sign-disclosure">
            {{ t("rasterEditor.sign.canvas") }}
            <small v-if="doc">{{ doc.width }} × {{ doc.height }}</small>
          </summary>
          <div class="sign-sizes">
            <button
              v-for="preset in decalSizes"
              :key="preset.name"
              :class="`sign-size-${preset.name}`"
              type="button"
              :disabled="locked"
              @click="createBlank(preset.width, preset.height)"
            >
              <span class="sign-size-shape" aria-hidden="true" />
              <span>{{ t(`rasterEditor.sign.sizes.${preset.name}`) }}</span>
              <small>{{ preset.width }} × {{ preset.height }}</small>
            </button>
          </div>
          <details class="sign-import source-section">
            <summary class="sign-disclosure">{{ t("rasterEditor.sign.sources") }}</summary>
            <RasterSourcePanel
              :image-width="doc?.width"
              :image-height="doc?.height"
              :profile="profile"
              :disabled="locked"
              @load-decal="loadDecal"
              @create="createBlank"
              @image="image"
              @schema="restore"
            />
          </details>
        </details>
        <RasterSignControls
v-if="isDecalStudio" :layer="layer" :settings="signSettings" :colors="decal.colors"
          :layers="decalLayers"
          :graffiti="isGraffiti" v-model:strength="sprayStrength"
          :disabled="locked" :can-letter="!!doc" @update:layer="onPickLayer" @settings="setSignSettings"
          @color="setSignColor" @lettering="addSignLettering" @layer-state="updateDecalLayer" />
        <section v-if="isLot" class="ground-inspector">
          <LotGroundSize
            :size="ground.size.value"
            :meters-per-pixel="sampling.metersPerPixel"
            :disabled="locked || !!lot.target"
            @apply="applyLotSize"
          />
          <LotMaterialPalette
            v-model:active="groundActive"
            :slots="groundSlots"
            :base-tile="lot.baseTile ?? 8"
            :atlas-png="groundAtlas"
            :locked="!!lot.target"
            :disabled="locked || !doc"
            :painted="ground.isSlotPainted"
            @update:slots="ground.setSlots"
            @update:base-tile="ground.setBaseTile"
            @remove="ground.remove"
          />
          <LotEdgeControls
            v-if="doc && groundSlots[groundActive]"
            v-model="lot"
            v-bind="{ slot: groundActive }"
            :disabled="locked"
            @change="changed"
          />
        </section>
        <section class="editor-controls">
          <h2>{{ t("rasterEditor.text171") }}</h2>
          <label v-if="isCircleTool">
            {{ t("rasterEditor.circleRadius") }}
            <input v-model.number="circleRadius" type="number" min="1" max="4096" step="1" />
          </label>
          <label v-if="tool !== 'circle-fill'">{{ t("rasterEditor.text113") }}<input v-model.number="size" type="number" min="1" max="256" /></label>
          <label v-if="tool === 'fill'">{{ t("rasterEditor.text114") }}<input v-model.number="tolerance" type="number" min="0" max="510" /></label>
          <label v-if="isDecalStudio && tool === 'fill'">
            {{ t("rasterEditor.decalLayers.fillThroughLocked") }}
            <Switch v-model="fillThroughLocked" :aria-label="t('rasterEditor.decalLayers.fillThroughLocked')" :disabled="locked" />
          </label>
          <template v-if="tool === 'parking'">
            <label>{{ t("rasterEditor.text115") }}<input v-model.number="parkingLength" type="number" min="1" max="256" /></label>
            <label>{{ t("rasterEditor.text116") }}<input v-model.number="parkingSpacing" type="number" min="1" max="256" /></label>
          </template>
          <template v-if="!isLot && !isDecalStudio">
            <label v-if="!mask && channel === 'rgba'">RGB <input v-model="color" type="color" /></label>
            <label v-if="channel === 'rgba' && (!mask || profile === 'decal-graffiti')">{{ t("rasterEditor.channelStrength") }}<input v-model.number="alpha" type="number" min="0" max="255" /></label>
            <label v-if="channel !== 'rgba'">{{ t("rasterEditor.channelValue", { channel: channel.toUpperCase() }) }}<input v-model.number="channelValue" type="number" min="0" max="255" /></label>
            <label>{{ t("rasterEditor.text122") }}<FSelect :model-value="channel" :options="['rgba','r','g','b','a'].map((value) => ({ value, label: value.toUpperCase() }))" @update:model-value="channel = $event as Channel" /></label>
          </template>
        </section>
        <section v-if="isDecalStudio && signInspectorOpen && previewDoc" class="sign-live">
          <h2>{{ t(isGraffiti ? "rasterEditor.graffiti.preview" : "rasterEditor.sign.preview") }}</h2>
          <GraffitiProjectionPreview
            v-if="isGraffiti" :doc="previewDoc" :version="version"
            :colors="decal.colors" :aspect-ratio="decal.aspectRatio"
          />
          <RasterDecalPreview
            v-else :doc="previewDoc" :version="version" :profile="profile"
            :colors="decal.colors" :aspect-ratio="decal.aspectRatio" :sign="signSettings"
          />
        </section>
        <p v-if="signAnimated" class="sign-export-note">{{ t("rasterEditor.sign.animatedExport") }}</p>
        <component v-if="profile !== 'slot0' && !signAnimated" :is="isDecalStudio ? 'details' : 'div'">
          <summary v-if="isDecalStudio" class="sign-disclosure">{{ t("rasterEditor.sign.advanced") }}</summary>
          <RasterMaterialPanel
          v-model:lot="lot"
          v-model:decal="decal"
          v-model:sampling="sampling"
          :profile="profile"
          :disabled="locked"
          :hide-decal-palette="isDecalStudio"
          @load-lot="loadLot"
          @load-decal="loadDecal"
          @changed="changed"
          />
        </component>
        <RasterPreviewPanel
          v-if="doc && profile !== 'slot0' && !isDecalStudio"
          :doc="doc"
          :version="version"
          :profile="profile"
          :colors="lot.colors"
          :material="lot"
          :decal-colors="decal.colors"
          :decal-aspect-ratio="decal.aspectRatio"
          :decal-preview-doc="previewDoc ?? undefined"
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
.actions:has(.sign-properties-toggle) { flex-wrap: wrap; }
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
.sign-layout {
  --sign-stage: #171f2c;
  --sign-grid: #8390ad18;
  grid-template-columns: minmax(0, 1fr) clamp(260px, 22%, 304px);
}
.sign-layout.sign-inspector-closed { grid-template-columns: minmax(0, 1fr); }
.sign-layout .workspace {
  border-inline: 0;
  overflow: auto;
}
.sign-inspector {
  --sign-inset: 16px;
  --sign-section-padding: 14px;
  --sign-group-gap: 16px;
  --sign-row-gap: 8px;
  --sign-control-height: 32px;
  --sign-control-radius: 6px;
  border-inline-start: 1px solid var(--border);
}
.sign-inspector :deep(.sign-control),
.sign-inspector :deep(.source-panel :is(input:not([type="file"]), button, .f-select)),
.sign-inspector :deep(.material-panel :is(input:not([type="checkbox"]), button, .f-select)) {
  box-sizing: border-box;
  min-inline-size: 0;
  min-block-size: var(--sign-control-height);
  padding: 5px 9px;
  border: 1px solid var(--border);
  border-radius: var(--sign-control-radius);
  background: var(--surface);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
}
.sign-inspector :deep(input.sign-control) { inline-size: 100%; }
.sign-inspector :deep(.sign-control:focus-visible) {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
.sign-inspector :deep(button.sign-control:enabled:hover) { background: var(--surface-hover); }
.sign-properties-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.sign-properties-toggle[aria-expanded="true"] { background: var(--accent); }
.sign-properties-toggle:hover,
.sign-disclosure:hover { background: var(--surface-hover); }
.sign-document-settings,
.sign-canvas-settings,
.sign-inspector :deep(.sign-controls),
.sign-live { border-block-end: 1px solid var(--border); }
.sign-document-fields {
  display: grid;
  gap: var(--sign-group-gap);
  padding: var(--sign-row-gap) var(--sign-inset) var(--sign-section-padding);
}
.sign-name-field {
  display: grid;
  gap: var(--sign-row-gap);
  font-size: 12px;
  font-weight: 600;
}
.sign-live { padding: var(--sign-section-padding) var(--sign-inset); }
.sign-live h2 { margin: 0 0 var(--sign-row-gap); font-size: 12px; font-weight: 600; }
.sign-sizes {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--sign-row-gap);
  padding: var(--sign-row-gap) var(--sign-inset);
}
.sign-sizes button {
  display: grid;
  grid-template-rows: 24px auto auto;
  place-items: center;
  gap: 4px;
  padding: 8px 3px;
  border-radius: var(--sign-control-radius);
}
.sign-size-shape {
  inline-size: 24px;
  block-size: 12px;
  border: 1px solid currentColor;
  border-radius: 2px;
  color: var(--muted-foreground);
}
.sign-size-tall .sign-size-shape { inline-size: 12px; block-size: 24px; }
.sign-size-square .sign-size-shape { inline-size: 18px; block-size: 18px; }
.sign-sizes small,
.sign-disclosure small { color: var(--muted-foreground); font-variant-numeric: tabular-nums; }
.sign-sizes button:hover:enabled, .sign-start button:hover:enabled { background: var(--accent); }
.sign-disclosure {
  padding: 10px var(--sign-inset);
  color: var(--foreground);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
.sign-disclosure small { float: inline-end; font-size: 11px; font-weight: 400; }
.sign-import .sign-disclosure { color: var(--muted-foreground); }
.sign-inspector :deep(:is(.source-panel, .material-panel)) {
  padding: var(--sign-row-gap) var(--sign-inset) var(--sign-section-padding);
  border-block-start: 0;
}
.sign-export-note {
  margin: 0;
  padding: var(--sign-section-padding) var(--sign-inset);
  color: var(--muted-foreground);
  font-size: 12px;
}
.empty-canvas.sign-empty {
  color: #e4e9f4;
  background: var(--sign-stage);
  background-image: radial-gradient(var(--sign-grid) 1px, transparent 1px);
  background-size: 16px 16px;
}
.sign-start { display: grid; justify-items: center; gap: 10px; padding: 24px; text-align: center; }
.sign-start strong { font-size: 17px; }
.sign-empty .sign-start p { margin: 0; color: #aebbd3; font-size: 12px; }
.sign-start button { margin-top: 4px; }
.sign-layout :is(button, summary):focus-visible { outline: 2px solid var(--primary); outline-offset: 2px; }
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
.left-rail .source-section :deep(.source-panel) {
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
.editor-layout:not(.sign-layout) .canvas-stage :deep(.raster-toolbar) {
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
.sign-layout .canvas-stage {
  flex-direction: column;
  min-block-size: 220px;
  padding: 0;
}
.sign-layout .canvas-stage :deep(.raster-toolbar) {
  flex: none;
  display: grid;
  justify-content: safe center;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
}
.sign-layout .canvas-stage :deep(.tools) { flex-wrap: wrap; }
.sign-layout .editor-canvas {
  min-height: 0;
  border: 0;
  border-radius: 0;
  background-color: var(--sign-stage);
}
.sign-layout .document-footer {
  flex: none;
  min-height: 24px;
  font-size: 10px;
}
.sign-layout .editor-guidance {
  margin: 0;
  padding: 5px 12px;
  border: 0;
  border-radius: 0;
  background: var(--surface);
  color: var(--muted-foreground);
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
.ground-inspector {
  display: grid;
  gap: 14px;
  padding: 14px;
  border-bottom: 1px solid var(--border);
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
.sign-inspector .editor-controls {
  gap: var(--sign-row-gap);
  padding: var(--sign-section-padding) var(--sign-inset);
}
.sign-inspector .editor-controls h2 {
  margin: 0;
  color: var(--foreground);
  font-size: 12px;
}
.sign-inspector .editor-controls label {
  gap: var(--sign-row-gap);
  font-size: 12px;
  white-space: normal;
}
.sign-inspector .editor-controls input {
  inline-size: 8ch;
  min-block-size: var(--sign-control-height);
  border-radius: var(--sign-control-radius);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.sign-inspector .editor-controls input:focus-visible { outline-offset: 1px; }
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
  .editor-layout:not(.sign-layout) {
    grid-template-columns: 190px minmax(0, 1fr);
  }
  .editor-layout.is-empty { grid-template-columns: minmax(0, 1fr); }
  .inspector:not(.sign-inspector) {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    border-top: 1px solid var(--border);
    max-height: 360px;
  }
}
@media (max-width: 760px) {
  .editor-layout.sign-layout {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(440px, 1fr) auto;
    overflow: auto;
  }
  .sign-inspector {
    border-inline-start: 0;
    border-block-start: 1px solid var(--border);
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
  .sign-layout .workspace { min-block-size: 440px; }
  .canvas-stage { min-height: 480px; }
  .sign-layout .canvas-stage { min-height: 360px; }
  .editor-layout:not(.sign-layout) .canvas-stage :deep(.raster-toolbar) {
    left: 50%;
    bottom: 42px;
    max-width: calc(100% - 24px);
  }
}
</style>
