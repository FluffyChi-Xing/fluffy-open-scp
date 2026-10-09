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
import FDropdownSelect from "@/components/ui/FSelect.vue";
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
}>();
const emit = defineEmits<{
  save: [schema: FlowSchema];
  dirty: [value: boolean];
  resize: [];
}>();
const config = ref({ ...props.schema.config });
watch(
  () => props.schema.config,
  (value, previous) => {
    if (JSON.stringify(value) === JSON.stringify(previous)) return;
    config.value = { ...value };
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
const materialIndex = computed({
  get: () => Number(config.value.materialIndex ?? 0),
  set: (value: number) => {
    config.value.materialIndex = value;
  },
});
const isTexture = computed(() => props.schema.kind === "texture-input");
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
        props.schemas.find((n) => n.id === config.value[s.id])?.config.asset ??
        "",
    ),
  async (paths) => {
    if (isTexture.value) return;
    const epoch = ++textureGeneration;
    try {
      const inputs = await Promise.all(
        paths.map((path) =>
          path ? request<ProjectAsset>("read-asset", { asset: path }) : null,
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
function save() {
  emit("save", { ...props.schema, config: { ...config.value } });
}
</script>
<template>
  <div class="asset-tool nodrag nowheel nopan">
    <p v-if="isTexture">{{ slot.hint }}</p>
    <button :disabled="busy || loading" @click="upload">
      {{ isTexture ? "上传" + slot.label : "上传模型 / 社区资产包" }}
    </button>
    <small v-if="asset"
      >{{ (asset.size / 1024).toFixed(1) }} KiB ·
      {{ asset.asset.split(".").at(-1)?.toUpperCase() }}</small
    >
    <img
      v-if="isTexture && textureUrl"
      :src="textureUrl"
      :alt="slot.label"
      class="texture-preview"
    />
    <pre v-else-if="isTexture && asset?.base64">{{ parameterPreview }}</pre>
    <template v-if="!isTexture">
      <FDropdownSelect
        v-if="properties.length"
        :disabled="Boolean(config.document)"
        :model-value="property ? key(property) : ''"
        @update:model-value="config.property = $event"
        :options="
          properties.map((t) => ({
            value: key(t),
            label: 'Property ' + key(t),
          }))
        "
      />
      <p v-if="config.document">编辑草稿已绑定当前 Property。</p>
      <AssetModelPreview :payload="payload" :glb="glb" />
      <p v-if="payload">
        模型预览；地块与组件请在 Property Editor 中查看和编辑。
      </p>
      <label v-if="rawPayload && rawPayload.materials.length > 1"
        >材质
        <input
          v-model.number="materialIndex"
          type="number"
          min="0"
          :max="rawPayload.materials.length - 1"
      /></label>
      <p v-if="glb">
        GLB 几何预览草稿；六槽游戏材质需要模型的立面 UV 和参数列属性。
      </p>
      <button
        :disabled="
          !property ||
          packageId == null ||
          Boolean(config.document && !savedDocument)
        "
        @click="editorOpen = true"
      >
        展开 Property Editor
      </button>
      <p v-if="asset?.resources">
        完整保留 {{ asset.resources.length }} 个原生资源及 TGI
      </p>
    </template>
    <button :disabled="busy" @click="save">保存节点与编辑 Schema</button>
    <p v-if="config.document && !savedDocument" role="alert">
      保存的 Schema 无效；请修复文档后再编辑。
    </p>
    <p v-if="error" role="alert">{{ error }}</p>
    <PropertyEditor
      v-if="editorOpen && packageId != null && property"
      :key="key(property)"
      v-model:open="editorOpen"
      :package-id="packageId"
      :tgi="property"
      :initial-schema="savedDocument"
      :preview-payload="payload"
      @schema-change="config.document = JSON.stringify($event)"
    />
  </div>
</template>
<style scoped>
.asset-tool {
  display: grid;
  gap: 10px;
  padding: 12px;
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
</style>
