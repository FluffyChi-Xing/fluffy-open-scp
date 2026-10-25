<script setup lang="ts">
import {
  computed,
  defineAsyncComponent,
  nextTick,
  ref,
  shallowRef,
  watch,
} from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import AssetModelPreview from "@/pages/packages/components/property-editor/AssetModelPreview.vue";
import { tauriApi } from "@/api";
import type { LotModelPayload, Tgi } from "@/api/tauri";
import { unitsFromSchema } from "@/pages/packages/components/property-editor/peSchemaDoc";
import { parseLotModelContainer } from "@/lib/three-gltf";
import type { FlowSchema } from "./contracts";
import {
  BUILDING_SLOTS,
  applyBuildingSlots,
  assetBytes,
  rasterDraftAsset,
  type ProjectAsset,
} from "./assetFlow";
const PropertyEditor = defineAsyncComponent(
  () =>
    import("@/pages/packages/components/property-editor/PropertyEditor.vue"),
);
const props = defineProps<{
  project: string;
  schema: FlowSchema;
  schemas: FlowSchema[];
  busy: boolean;
  embedded?: boolean;
}>();
const emit = defineEmits<{
  save: [schema: FlowSchema];
  dirty: [value: boolean];
  resize: [];
}>();
const config = ref({ ...props.schema.config });
const { t } = useI18n();
watch(
  () => props.schema.config,
  (value, previous) => {
    if (JSON.stringify(value) === JSON.stringify(previous)) return;
    config.value = { ...value };
    syncDraftFromConfig();
  },
);
watch(
  config,
  (value) =>
    emit(
      "dirty",
      JSON.stringify(value) !== JSON.stringify(props.schema.config),
    ),
  { deep: true },
);
const error = shallowRef(""),
  loading = shallowRef(false),
  editorOpen = shallowRef(false);
const asset = shallowRef<ProjectAsset | null>(null),
  rawPayload = shallowRef<LotModelPayload | null>(null),
  glb = shallowRef<ArrayBuffer | null>(null);
const payload = shallowRef<LotModelPayload | null>(null);
const packageId = shallowRef<number | null>(null),
  resources = shallowRef<Tgi[]>([]);
const textureInputs = shallowRef<(ProjectAsset | null)[]>([]);
type BindingKind = "lot" | "props" | "light" | "effect" | "spawner";
type PanelId = "components" | "slots" | "schema" | "inspector" | "menu";
interface AssetBinding {
  id: string;
  kind: BindingKind;
  name: string;
  enabled: boolean;
  position: [number, number, number];
}
const bindingKinds: { kind: BindingKind; icon: string; label: string }[] = [
  { kind: "lot", icon: "LandPlot", label: "studio.assetWorkbench.editor.lot" },
  { kind: "props", icon: "Box", label: "studio.assetWorkbench.editor.props" },
  { kind: "light", icon: "Lightbulb", label: "studio.assetWorkbench.editor.light" },
  { kind: "effect", icon: "Sparkles", label: "studio.assetWorkbench.editor.effect" },
  { kind: "spawner", icon: "MapPin", label: "studio.assetWorkbench.editor.spawner" },
];
const bindings = ref<AssetBinding[]>([]);
const selectedBindingId = ref<string | null>(null);
const activePanel = ref<PanelId>("components");
const sidebarPanels: { id: PanelId; label: string }[] = [
  { id: "components", label: "studio.assetWorkbench.editor.components" },
  { id: "slots", label: "studio.assetWorkbench.editor.slots" },
  { id: "schema", label: "studio.assetWorkbench.editor.schema" },
  { id: "menu", label: "studio.assetWorkbench.editor.menu" },
  { id: "inspector", label: "studio.assetWorkbench.editor.inspector" },
];
const fullscreen = ref(false);
const menuBinding = ref({ enabled: false, label: "", category: "" });
const slotAssets = ref<(ProjectAsset | null)[]>(Array.from({ length: 6 }, () => null));
const materialIndex = computed({
  get: () => Number(config.value.materialIndex ?? 0),
  set: (value: number) => {
    config.value.materialIndex = value;
  },
});
const isTexture = computed(() => props.schema.kind === "texture-input");
const nativeEditorActive = computed(() => !isTexture.value && Boolean(editorOpen.value && packageId.value != null && property.value));
const activeBinding = computed(() => bindings.value.find((item) => item.id === selectedBindingId.value) ?? null);
const bindingCount = computed(() => bindings.value.filter((item) => item.enabled).length);
const slot = computed(() => BUILDING_SLOTS[Number(config.value.slot) || 0]);
const textureUrl = computed(() =>
  asset.value?.base64 && asset.value.asset.endsWith(".png")
    ? `data:image/png;base64,${asset.value.base64}`
    : "",
);
const parameterPreview = computed(() =>
  asset.value?.base64
    ? new TextDecoder().decode(assetBytes(asset.value)).slice(0, 400)
    : "",
);
const key = (t: Tgi) =>
  [t.typeId, t.group, t.instance]
    .map((n) => n.toString(16).padStart(8, "0"))
    .join(":");
const properties = computed(() =>
  resources.value.filter((t) => t.typeId === 0x00b1b104),
);
const property = computed(
  () =>
    properties.value.find((t) => key(t) === config.value.property) ??
    (!config.value.property ? properties.value[0] : null) ??
    null,
);
watch([property, packageId], ([nextProperty, nextPackage]) => {
  if (props.embedded && nextProperty && nextPackage != null) editorOpen.value = true;
});
const savedDocument = computed(() => {
  try {
    if (!config.value.document) return null;
    const document = JSON.parse(String(config.value.document)) as Record<
      string,
      unknown
    >;
    unitsFromSchema(document);
    if (
      !document.lot ||
      !Array.isArray((document.lot as Record<string, unknown>).colors)
    )
      return null;
    return document;
  } catch {
    return null;
  }
});
let generation = 0,
  nativeGeneration = 0,
  textureGeneration = 0;
let focusAfterLoad = false;
const request = <T,>(action: string, payload: unknown) =>
  invoke<T>("code_flow", {
    request: { project: props.project, action, payload },
  });
async function loadAsset() {
  const epoch = ++generation;
  ++nativeGeneration;
  packageId.value = null;
  resources.value = [];
  if (!config.value.asset) {
    loading.value = false;
    error.value = "";
    asset.value = null;
    rawPayload.value = null;
    payload.value = null;
    glb.value = null;
    return;
  }
  loading.value = true;
  error.value = "";
  try {
    const value = await request<ProjectAsset>("read-asset", {
      asset: config.value.asset,
    });
    if (epoch !== generation) return;
    asset.value = value;
    rawPayload.value = null;
    payload.value = null;
    glb.value = null;
    if (!isTexture.value && value.resources) {
      const opened = await tauriApi.packages.open(value.path);
      if (epoch !== generation) return;
      packageId.value = opened.package.packageId;
      resources.value = value.resources;
      await loadNativeModel();
    } else if (value.asset.endsWith(".lotm"))
      rawPayload.value = parseLotModelContainer(assetBytes(value).buffer);
    else if (value.asset.endsWith(".glb")) glb.value = assetBytes(value).buffer;
    applySlots();
  } catch (cause) {
    if (epoch === generation) error.value = String(cause);
  } finally {
    if (epoch === generation) {
      loading.value = false;
      if (focusAfterLoad) {
        focusAfterLoad = false;
        await nextTick();
        emit("resize");
      }
    }
  }
}
async function loadNativeModel() {
  const epoch = ++nativeGeneration;
  if (packageId.value == null) return;
  const pkg = packageId.value;
  const selected = property.value;
  let model = resources.value.find((t) => t.typeId === 0x2f4e681b);
  let modelPackage = pkg;
  if (selected) {
    const session = await tauriApi.packages.readLotEditorSession(pkg, selected);
    const lod = session.modelLods.find((l) => l);
    if (lod) {
      model = lod.tgi;
      modelPackage = lod.packageId;
    }
  }
  if (epoch !== nativeGeneration) return;
  const bytes = model
    ? await tauriApi.packages.readLotModelMeshes(modelPackage, model)
    : null;
  if (epoch !== nativeGeneration) return;
  rawPayload.value = bytes ? parseLotModelContainer(bytes) : null;
  applySlots();
}
function applySlots() {
  if (!rawPayload.value) {
    payload.value = null;
    return;
  }
  try {
    payload.value = applyBuildingSlots(
      rawPayload.value,
      textureInputs.value,
      materialIndex.value,
    );
    error.value = "";
  } catch (cause) {
    error.value = String(cause);
    payload.value = rawPayload.value;
  }
}
watch(() => [props.project, config.value.asset], loadAsset, {
  immediate: true,
});
watch(materialIndex, applySlots);
watch(
  () => config.value.property,
  () => {
    void loadNativeModel().catch((cause) => {
      error.value = String(cause);
    });
  },
);
watch(
  () =>
    BUILDING_SLOTS.map(
      (s) =>
        props.schemas.find((n) => n.id === config.value[s.id])?.config,
    ),
  async (paths) => {
    if (isTexture.value) return;
    const epoch = ++textureGeneration;
    try {
      const inputs = await Promise.all(
        paths.map((input, index) =>
          input?.document ? rasterDraftAsset(String(input.document), index) :
          input?.asset ? request<ProjectAsset>("read-asset", { asset: input.asset }) : null,
        ),
      );
      if (epoch !== textureGeneration) return;
      textureInputs.value = inputs;
      applySlots();
    } catch (cause) {
      error.value = String(cause);
    }
  },
  { immediate: true },
);
async function upload() {
  const path = await open({
    multiple: false,
    filters: [
      {
        name: isTexture.value ? slot.value.label : "模型 / 原生资产包",
        extensions: isTexture.value
          ? [...slot.value.accept]
          : ["package", "glb", "lotm"],
      },
    ],
  });
  if (typeof path !== "string") return;
  loading.value = true;
  try {
    const value = await request<ProjectAsset>("import-asset", { path });
    focusAfterLoad = true;
    config.value = {
      ...config.value,
      asset: value.asset,
      ...(isTexture.value ? {} : { document: "", property: "" }),
    };
    emit("save", { ...props.schema, config: { ...config.value } });
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}
async function uploadSlot(index: number) {
  const slotInfo = BUILDING_SLOTS[index];
  const path = await open({ multiple: false, filters: [{ name: slotInfo.label, extensions: [...slotInfo.accept] }] });
  if (typeof path !== "string") return;
  try {
    const value = await request<ProjectAsset>("import-asset", { path });
    slotAssets.value = slotAssets.value.map((item, slotIndex) => slotIndex === index ? value : item);
    config.value = { ...config.value, [`materialSlot${index}`]: value.asset };
    emit("dirty", true);
  } catch (cause) {
    error.value = String(cause);
  }
}
function parseBindings(value: unknown): AssetBinding[] {
  if (typeof value !== "string") return [];
  try {
    const parsed = JSON.parse(value) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((item): item is AssetBinding => {
      if (!item || typeof item !== "object") return false;
      const row = item as Record<string, unknown>;
      return typeof row.id === "string" && typeof row.kind === "string" && typeof row.name === "string";
    });
  } catch {
    return [];
  }
}
function syncDraftFromConfig() {
  const saved = parseBindings(config.value.bindings);
  bindings.value = saved.length ? saved : [{ id: "lot-1", kind: "lot", name: t("studio.assetWorkbench.editor.defaultLot"), enabled: true, position: [0, 0, 0] }];
  selectedBindingId.value = bindings.value[0]?.id ?? null;
  try {
    const menu = JSON.parse(String(config.value.menu ?? "{}")) as Record<string, unknown>;
    menuBinding.value = { enabled: Boolean(menu.enabled), label: String(menu.label ?? ""), category: String(menu.category ?? "") };
  } catch {
    menuBinding.value = { enabled: false, label: "", category: "" };
  }
  slotAssets.value = slotAssets.value.map((item, index) => item ?? (config.value[`materialSlot${index}`] ? { asset: String(config.value[`materialSlot${index}`]), path: "", size: 0 } : null));
}
function addBinding(kind: BindingKind) {
  const index = bindings.value.filter((item) => item.kind === kind).length + 1;
  const id = `${kind}-${Date.now()}-${index}`;
  bindings.value = [...bindings.value, { id, kind, name: t(`studio.assetWorkbench.editor.${kind}`), enabled: true, position: [0, 0, 0] }];
  selectedBindingId.value = id;
  activePanel.value = "inspector";
}
function updateActiveBinding(patch: Partial<AssetBinding>) {
  if (!selectedBindingId.value) return;
  bindings.value = bindings.value.map((item) => item.id === selectedBindingId.value ? { ...item, ...patch } : item);
}
function save() {
  config.value = {
    ...config.value,
    bindings: JSON.stringify(bindings.value),
    menu: JSON.stringify(menuBinding.value),
    ...Object.fromEntries(slotAssets.value.map((item, index) => [`materialSlot${index}`, item?.asset ?? config.value[`materialSlot${index}`] ?? ""])),
  };
  emit("save", { ...props.schema, config: { ...config.value } });
}
function toggleFullscreen() {
  fullscreen.value = !fullscreen.value;
}
syncDraftFromConfig();
</script>
<template>
  <section class="asset-tool nodrag nowheel nopan" :class="{ 'asset-tool-embedded': props.embedded, fullscreen }" :aria-label="t('studio.assetWorkbench.editor.title')">
    <header v-if="!isTexture" class="asset-editor-toolbar">
      <div class="asset-editor-title">
        <span class="asset-editor-eyebrow">{{ t('studio.assetWorkbench.editor.eyebrow') }}</span>
        <strong>{{ asset?.asset?.split('/').at(-1) ?? t('studio.assetWorkbench.editor.emptyAsset') }}</strong>
        <small v-if="asset">{{ (asset.size / 1024).toFixed(1) }} KiB · {{ asset.asset.split('.').at(-1)?.toUpperCase() }}</small>
      </div>
      <div class="asset-editor-actions">
        <button type="button" class="editor-icon-button" :disabled="busy || loading" :title="t('studio.assetWorkbench.editor.upload')" :aria-label="t('studio.assetWorkbench.editor.upload')" @click="upload"><FIcon name="Upload" :size="14" aria-label="" /></button>
        <button type="button" class="editor-icon-button" :disabled="busy || !asset" :title="t('studio.assetWorkbench.editor.save')" :aria-label="t('studio.assetWorkbench.editor.save')" @click="save"><FIcon name="Save" :size="14" aria-label="" /></button>
        <button type="button" class="editor-icon-button" :title="t(fullscreen ? 'studio.assetWorkbench.editor.exitFullscreen' : 'studio.assetWorkbench.editor.fullscreen')" :aria-label="t(fullscreen ? 'studio.assetWorkbench.editor.exitFullscreen' : 'studio.assetWorkbench.editor.fullscreen')" @click="toggleFullscreen"><FIcon :name="fullscreen ? 'Minimize2' : 'Maximize2'" :size="14" aria-label="" /></button>
      </div>
    </header>

    <div v-if="isTexture" class="texture-input">
      <p>{{ slot.hint }}</p>
      <button type="button" :disabled="busy || loading" @click="upload">{{ t('studio.assetWorkbench.editor.uploadSlot', { slot: slot.label }) }}</button>
      <img v-if="textureUrl" :src="textureUrl" :alt="slot.label" class="texture-preview" />
      <pre v-else-if="asset?.base64">{{ parameterPreview }}</pre>
    </div>

    <template v-else>
      <div class="asset-editor-layout">
        <main class="asset-editor-canvas">
          <div class="asset-canvas-toolbar" role="toolbar" :aria-label="t('studio.assetWorkbench.editor.canvasToolbar')">
            <span><FIcon name="Box" :size="14" aria-label="" /> {{ t('studio.assetWorkbench.editor.canvas') }}</span>
            <button v-if="!nativeEditorActive" type="button" :disabled="!asset" @click="upload"><FIcon name="Upload" :size="13" aria-label="" /> {{ t('studio.assetWorkbench.editor.importModel') }}</button>
            <button v-if="property && packageId != null && !editorOpen" type="button" @click="editorOpen = true"><FIcon name="SlidersHorizontal" :size="13" aria-label="" /> {{ t('studio.assetWorkbench.editor.openPropertyEditor') }}</button>
            <span v-if="bindingCount" class="canvas-status">{{ t('studio.assetWorkbench.editor.bindingCount', { count: bindingCount }) }}</span>
          </div>
          <div v-if="nativeEditorActive" class="property-editor-host">
            <PropertyEditor :key="key(property!)" v-model:open="editorOpen" :package-id="packageId!" :tgi="property!" :initial-schema="savedDocument" :preview-payload="payload" mode="editor" embedded external-toolbar external-sidebar @schema-change="config.document = JSON.stringify($event)" />
          </div>
          <div v-else class="draft-editor-canvas">
            <AssetModelPreview v-if="payload || glb" :payload="payload" :glb="glb" />
            <div v-else class="asset-default-canvas" :aria-label="t('studio.code.defaultAssetCanvas')"><div class="asset-default-grid" aria-hidden="true"></div><div class="asset-default-copy"><FIcon name="Box" :size="28" aria-label="" /><strong>{{ t('studio.assetWorkbench.editor.emptyCanvas') }}</strong><span>{{ t('studio.assetWorkbench.editor.emptyCanvasHint') }}</span></div></div>
            <div v-if="loading" class="draft-overlay" role="status">{{ t('common.loading') }}</div>
            <div v-if="error" class="draft-error" role="alert">{{ error }}</div>
          </div>
        </main>

        <aside class="asset-editor-sidebar">
          <div class="asset-sidebar-tabs" role="tablist" :aria-label="t('studio.assetWorkbench.editor.sidebar')">
            <button v-for="panel in sidebarPanels" :key="panel.id" type="button" role="tab" :aria-selected="activePanel === panel.id" :class="{ active: activePanel === panel.id }" @click="activePanel = panel.id">{{ t(panel.label) }}</button>
          </div>
          <div v-if="activePanel === 'components'" class="sidebar-scroll">
            <div class="sidebar-heading"><strong>{{ t('studio.assetWorkbench.editor.componentTree') }}</strong><span>{{ bindings.length }}</span></div>
            <button v-for="binding in bindings" :key="binding.id" type="button" class="binding-row" :class="{ active: binding.id === selectedBindingId }" @click="selectedBindingId = binding.id; activePanel = 'inspector'"><FIcon :name="bindingKinds.find((item) => item.kind === binding.kind)?.icon ?? 'Box'" :size="14" aria-label="" /><span>{{ binding.name }}</span><small>{{ t(`studio.assetWorkbench.editor.${binding.kind}`) }}</small></button>
            <div class="binding-add-grid"><button v-for="entry in bindingKinds" :key="entry.kind" type="button" @click="addBinding(entry.kind)"><FIcon :name="entry.icon" :size="14" aria-label="" />{{ t(entry.label) }}</button></div>
          </div>
          <div v-else-if="activePanel === 'slots'" class="sidebar-scroll">
            <div class="sidebar-heading"><strong>{{ t('studio.assetWorkbench.editor.sixSlots') }}</strong><span>{{ slotAssets.filter(Boolean).length }}/6</span></div>
            <button v-for="(slotInfo, index) in BUILDING_SLOTS" :key="slotInfo.id" type="button" class="slot-row" @click="uploadSlot(index)"><span class="slot-index">{{ index }}</span><span><strong>{{ slotInfo.label }}</strong><small>{{ slotAssets[index]?.asset?.split('/').at(-1) ?? t('studio.assetWorkbench.editor.slotEmpty') }}</small></span><FIcon :name="slotAssets[index] ? 'CheckCircle2' : 'Upload'" :size="14" aria-label="" /></button>
          </div>
          <div v-else-if="activePanel === 'schema'" class="sidebar-scroll schema-panel"><div class="sidebar-heading"><strong>{{ t('studio.assetWorkbench.editor.schemaTitle') }}</strong></div><pre>{{ config.document || t('studio.assetWorkbench.editor.schemaEmpty') }}</pre></div>
          <div v-else-if="activePanel === 'menu'" class="sidebar-scroll menu-panel"><div class="sidebar-heading"><strong>{{ t('studio.assetWorkbench.editor.menuBinding') }}</strong></div><label class="check-row"><input v-model="menuBinding.enabled" type="checkbox" />{{ t('studio.assetWorkbench.editor.menuEnabled') }}</label><label>{{ t('studio.assetWorkbench.editor.menuLabel') }}<input v-model="menuBinding.label" type="text" :placeholder="t('studio.assetWorkbench.editor.menuLabelPlaceholder')" /></label><label>{{ t('studio.assetWorkbench.editor.menuCategory') }}<input v-model="menuBinding.category" type="text" :placeholder="t('studio.assetWorkbench.editor.menuCategoryPlaceholder')" /></label><p>{{ t('studio.assetWorkbench.editor.menuHint') }}</p><RouterLink class="menu-editor-link" to="/studio/ui">{{ t('studio.assetWorkbench.editor.openMenuEditor') }} <FIcon name="ExternalLink" :size="12" aria-label="" /></RouterLink></div>
          <div v-else class="sidebar-scroll inspector-panel">
            <div class="sidebar-heading"><strong>{{ t('studio.assetWorkbench.editor.inspectorTitle') }}</strong></div>
            <template v-if="activeBinding"><label>{{ t('studio.assetWorkbench.editor.name') }}<input :value="activeBinding.name" type="text" @input="updateActiveBinding({ name: ($event.target as HTMLInputElement).value })" /></label><label class="check-row"><input :checked="activeBinding.enabled" type="checkbox" @change="updateActiveBinding({ enabled: ($event.target as HTMLInputElement).checked })" />{{ t('studio.assetWorkbench.editor.enabled') }}</label><div class="position-fields"><label>{{ t('studio.assetWorkbench.editor.axisX') }}<input :value="activeBinding.position[0]" type="number" @input="updateActiveBinding({ position: [Number(($event.target as HTMLInputElement).value), activeBinding.position[1], activeBinding.position[2]] })" /></label><label>{{ t('studio.assetWorkbench.editor.axisY') }}<input :value="activeBinding.position[1]" type="number" @input="updateActiveBinding({ position: [activeBinding.position[0], Number(($event.target as HTMLInputElement).value), activeBinding.position[2]] })" /></label><label>{{ t('studio.assetWorkbench.editor.axisZ') }}<input :value="activeBinding.position[2]" type="number" @input="updateActiveBinding({ position: [activeBinding.position[0], activeBinding.position[1], Number(($event.target as HTMLInputElement).value)] })" /></label></div></template><p v-else>{{ t('studio.assetWorkbench.editor.selectBinding') }}</p>
          </div>
        </aside>
      </div>
      <footer class="asset-editor-footer"><span v-if="asset?.resources">{{ t('studio.assetWorkbench.editor.nativeResources', { count: asset.resources.length }) }}</span><span v-else>{{ t('studio.assetWorkbench.editor.draftNotice') }}</span><button type="button" :disabled="busy" @click="save"><FIcon name="Save" :size="13" aria-label="" />{{ t('studio.assetWorkbench.editor.saveNode') }}</button></footer>
    </template>
  </section>
</template>
<style scoped>
.asset-tool {
  background: var(--surface);
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
}
.asset-tool-embedded {
  width: 100%;
}
.asset-tool.fullscreen {
  inset: 12px;
  position: fixed;
  z-index: 90;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-xl);
}
.asset-tool p,
.asset-tool small {
  font-size: 11px;
  color: var(--muted-foreground);
  margin: 0;
}
.asset-tool button {
  padding: 7px 10px;
  border: 1px solid var(--border);
  border-radius: 5px;
  color: var(--foreground);
  background: var(--surface);
  cursor: pointer;
}
.asset-tool button:disabled {
  opacity: 0.5;
  cursor: default;
}
.asset-editor-toolbar { align-items: center; border-bottom: 1px solid var(--border); display: flex; flex: 0 0 auto; justify-content: space-between; gap: 16px; min-height: 52px; padding: 8px 12px; }
.asset-editor-title { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.asset-editor-eyebrow { color: var(--primary); font-size: 10px; letter-spacing: .08em; text-transform: uppercase; }
.asset-editor-title strong { font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.asset-editor-title small { color: var(--muted-foreground); font-size: 10px; }
.asset-editor-actions { display: flex; gap: 5px; }
.editor-icon-button { align-items: center; display: inline-flex; height: 28px; justify-content: center; padding: 0 !important; width: 28px; }
.asset-editor-layout { display: grid; flex: 1; grid-template-columns: minmax(0, 1fr) minmax(260px, 320px); min-height: 0; }
.asset-editor-canvas { background: color-mix(in srgb, var(--surface) 88%, var(--primary)); display: flex; flex-direction: column; min-height: 0; min-width: 0; }
.asset-canvas-toolbar { align-items: center; border-bottom: 1px solid var(--border); display: flex; flex: 0 0 auto; gap: 8px; min-height: 36px; padding: 5px 10px; }
.asset-canvas-toolbar > span:first-child { align-items: center; display: inline-flex; flex: 1; font-size: 11px; gap: 5px; }
.asset-canvas-toolbar button { align-items: center; display: inline-flex; font-size: 10px; gap: 5px; padding: 4px 7px; }
.canvas-status { color: var(--primary); flex: 0 0 auto !important; font-size: 10px !important; }
.property-editor-host, .draft-editor-canvas { flex: 1; min-height: 0; min-width: 0; position: relative; }
.property-editor-host :deep(.embedded-root), .property-editor-host :deep(.editor-root) { height: 100%; }
.draft-editor-canvas :deep(.asset-preview) { border-radius: 0; height: 100%; min-height: 0; }
.asset-default-canvas { align-items: center; background: #0e1a31; display: flex; flex: 1; justify-content: center; min-height: 0; overflow: hidden; position: relative; }
.asset-default-grid { background-image: linear-gradient(color-mix(in srgb, var(--primary) 22%, transparent) 1px, transparent 1px), linear-gradient(90deg, color-mix(in srgb, var(--primary) 22%, transparent) 1px, transparent 1px); background-size: 28px 28px; inset: 0; opacity: .7; position: absolute; transform: perspective(420px) rotateX(58deg) scale(1.5); transform-origin: center bottom; }
.asset-default-copy { align-items: center; color: var(--muted-foreground); display: flex; flex-direction: column; gap: 7px; max-width: 280px; position: relative; text-align: center; }
.asset-default-copy strong { color: var(--foreground); font-size: 13px; }
.asset-default-copy span { font-size: 11px; line-height: 1.5; }
.draft-overlay { align-items: center; background: color-mix(in srgb, var(--surface) 80%, transparent); display: flex; inset: 0; justify-content: center; position: absolute; }
.draft-error { background: color-mix(in srgb, var(--destructive) 15%, var(--surface)); bottom: 10px; color: var(--destructive); font-size: 11px; left: 10px; padding: 6px 8px; position: absolute; right: 10px; }
.asset-editor-sidebar { border-inline-start: 1px solid var(--border); display: flex; flex-direction: column; min-height: 0; min-width: 0; }
.asset-sidebar-tabs { border-bottom: 1px solid var(--border); display: flex; flex-wrap: wrap; gap: 2px; padding: 5px; }
.asset-sidebar-tabs button { background: transparent; border: 0; border-bottom: 2px solid transparent; font-size: 10px; padding: 6px 7px; white-space: nowrap; }
.asset-sidebar-tabs button.active { border-color: var(--primary); color: var(--foreground); }
.sidebar-scroll { flex: 1; min-height: 0; overflow: auto; padding: 10px; }
.sidebar-heading { align-items: center; color: var(--muted-foreground); display: flex; font-size: 10px; justify-content: space-between; letter-spacing: .05em; margin-bottom: 8px; text-transform: uppercase; }
.sidebar-heading span { color: var(--primary); }
.binding-row, .slot-row { align-items: center; display: flex !important; gap: 7px; justify-content: flex-start; margin-bottom: 4px; padding: 7px 8px !important; text-align: start; width: 100%; }
.binding-row span, .slot-row > span:not(.slot-index) { flex: 1; min-width: 0; }
.binding-row small, .slot-row small { color: var(--muted-foreground); display: block; font-size: 9px; margin-top: 2px; }
.binding-row.active { border-color: var(--primary); background: color-mix(in srgb, var(--primary) 12%, var(--surface)); }
.binding-add-grid { display: grid; gap: 5px; grid-template-columns: repeat(2, minmax(0, 1fr)); margin-top: 12px; }
.binding-add-grid button { align-items: center; display: flex !important; font-size: 10px; gap: 4px; justify-content: flex-start; padding: 6px !important; }
.slot-index { align-items: center; background: var(--surface-hover); border-radius: 4px; display: inline-flex; flex: 0 0 auto; font-size: 10px; height: 22px; justify-content: center; width: 22px; }
.schema-panel pre { color: var(--muted-foreground); font: 10px/1.5 var(--font-mono, monospace); margin: 0; white-space: pre-wrap; }
.menu-panel, .inspector-panel { display: flex; flex-direction: column; gap: 10px; }
.menu-panel label, .inspector-panel label { color: var(--muted-foreground); display: flex; flex-direction: column; font-size: 10px; gap: 5px; }
.menu-panel input, .inspector-panel input { background: var(--surface-elevated); border: 1px solid var(--border); border-radius: var(--radius-sm); color: var(--foreground); font: inherit; min-height: 26px; padding: 4px 6px; }
.menu-editor-link { align-items: center; border: 1px solid var(--border); border-radius: var(--radius-sm); color: var(--foreground); display: inline-flex; font-size: 10px; gap: 4px; justify-content: center; padding: 6px; text-decoration: none; }
.check-row { align-items: center !important; flex-direction: row !important; }
.position-fields { display: grid; gap: 5px; grid-template-columns: repeat(3, 1fr); }
.asset-editor-footer { align-items: center; border-top: 1px solid var(--border); color: var(--muted-foreground); display: flex; flex: 0 0 auto; font-size: 10px; gap: 10px; justify-content: space-between; min-height: 32px; padding: 4px 10px; }
.asset-editor-footer button { align-items: center; display: inline-flex; font-size: 10px; gap: 4px; padding: 4px 7px; }
.texture-input { display: grid; gap: 10px; padding: 12px; }
.texture-preview {
  width: 100%;
  height: 110px;
  object-fit: contain;
  background: repeating-conic-gradient(#ddd 0% 25%, #999 0% 50%) 0 / 16px 16px;
  image-rendering: auto;
}
.asset-tool pre {
  max-height: 110px;
  overflow: auto;
  font-size: 10px;
  white-space: pre-wrap;
}
.asset-tool p[role="alert"] {
  color: var(--destructive);
}
@media (max-width: 860px) {
  .asset-editor-layout { grid-template-columns: minmax(0, 1fr) 250px; }
  .asset-sidebar-tabs button { padding-inline: 5px; }
}
</style>
