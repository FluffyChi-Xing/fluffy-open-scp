<script setup lang="ts">
import { computed, defineAsyncComponent, ref, toRef } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import MapSourceTool from "./mod-flow/MapSourceTool.vue";
import AssetFlowTool from "./mod-flow/AssetFlowTool.vue";
import { useModFlow } from "./mod-flow/useModFlow";
import type { FlowSchema } from "./mod-flow/contracts";
import type { Tgi } from "@/api/tauri";

const PropertyEditor = defineAsyncComponent(
  () => import("@/pages/packages/components/property-editor/PropertyEditor.vue"),
);

const props = defineProps<{
  project: string;
  kind: string;
  manifest: Record<string, unknown>;
}>();
const emit = defineEmits<{ focus: []; fullscreen: [] }>();
const { t } = useI18n();
const propertyOpen = ref(true);
const fullscreen = ref(false);
const mapPackagePath = ref("");
const mapGroup = ref("");
const flow = useModFlow(toRef(props, "project"));
const flowBusy = computed(() => flow.busy.value);
const assetSchema = computed<FlowSchema | null>(() => flow.state.value?.schemas.find((schema) => schema.kind === "building-asset") ?? null);
const assetSchemas = computed(() => flow.state.value?.schemas ?? []);

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" ? value as Record<string, unknown> : null;
}

function parseTgi(value: unknown): Tgi | null {
  if (typeof value === "string") {
    const parts = value.split(/[:/,]/).map((part) => Number.parseInt(part.trim().replace(/^0x/i, ""), 16));
    if (parts.length === 3 && parts.every(Number.isFinite)) return { typeId: parts[0], group: parts[1], instance: parts[2] };
  }
  const record = asRecord(value);
  if (!record) return null;
  const typeId = Number(record.typeId ?? record.type_id);
  const group = Number(record.group);
  const instance = Number(record.instance);
  return [typeId, group, instance].every(Number.isFinite) ? { typeId, group, instance } : null;
}

const assetTarget = computed(() => {
  const source = asRecord(props.manifest.asset) ?? asRecord(props.manifest.asset_target) ?? props.manifest;
  const packageId = Number(source?.packageId ?? source?.package_id);
  const tgi = parseTgi(source?.tgi ?? source?.targetTgi ?? source?.target_tgi);
  return Number.isFinite(packageId) && tgi ? { packageId, tgi } : null;
});

const editorTitle = computed(() => {
  if (props.kind === "map") return t("studio.code.mapPreview");
  if (props.kind === "assets") return t("studio.code.assetEditor");
  return t("studio.code.editorPanel");
});
</script>

<template>
  <section class="standard-editor" :class="{ fullscreen }" :aria-label="editorTitle">
    <header v-if="kind !== 'assets'" class="standard-editor-head">
      <div>
        <p class="eyebrow">{{ t("studio.code.standardWorkbench") }}</p>
        <h2>{{ editorTitle }}</h2>
      </div>
      <div class="editor-actions">
        <button type="button" class="editor-icon" :title="t('studio.code.focusEditor')" :aria-label="t('studio.code.focusEditor')" @click="emit('focus')"><FIcon name="PanelTop" :size="14" aria-label="" /></button>
        <button type="button" class="editor-icon" :title="t('studio.code.fullscreenEditor')" :aria-label="t('studio.code.fullscreenEditor')" @click="fullscreen = !fullscreen"><FIcon :name="fullscreen ? 'Minimize2' : 'Maximize2'" :size="14" aria-label="" /></button>
        <span class="editor-kind">{{ kind }}</span>
      </div>
    </header>

    <div v-if="kind === 'map'" class="editor-content map-content">
      <MapSourceTool v-model:package-path="mapPackagePath" v-model:group="mapGroup" :disabled="false" />
    </div>
    <div v-else-if="kind === 'assets'" class="editor-content asset-content">
      <AssetFlowTool
        v-if="assetSchema"
        :project="project"
        :schema="assetSchema"
        :schemas="assetSchemas"
        :busy="flowBusy"
        embedded
        @save="flow.save"
      />
      <p v-else-if="assetTarget && !propertyOpen" class="editor-description">{{ t("studio.code.assetEditorHint") }}</p>
      <button v-if="assetTarget && !propertyOpen" type="button" class="editor-action" @click="propertyOpen = true">
        {{ t("studio.code.openAssetEditor") }}
      </button>
      <div v-if="!assetTarget && !assetSchema" class="default-asset-canvas">
        <div class="default-grid" aria-hidden="true"></div>
        <div class="default-canvas-copy">
          <span class="default-canvas-icon">◇</span>
          <strong>{{ t("studio.code.defaultAssetCanvas") }}</strong>
          <p>{{ t("studio.code.assetEditorUnavailable") }}</p>
        </div>
      </div>
      <PropertyEditor
        v-if="assetTarget && !assetSchema"
        v-model:open="propertyOpen"
        :package-id="assetTarget.packageId"
        :tgi="assetTarget.tgi"
        mode="editor"
        embedded
      />
    </div>
    <div v-else class="editor-content editor-empty">
      {{ t("studio.code.editorPending") }}
    </div>
  </section>
</template>

<style scoped>
.standard-editor { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--surface); }
.standard-editor.fullscreen { inset: 12px; position: fixed; z-index: 90; border: 1px solid var(--border); border-radius: 8px; box-shadow: var(--shadow-xl); }
.standard-editor-head { display: flex; align-items: center; justify-content: space-between; padding: 12px 14px; border-bottom: 1px solid var(--border); flex: 0 0 auto; }
.standard-editor-head h2 { margin: 2px 0 0; font-size: 14px; font-weight: 600; }
.eyebrow { margin: 0; color: var(--muted-foreground); font-size: 10px; text-transform: uppercase; letter-spacing: .08em; }
.editor-kind { color: var(--muted-foreground); font: 11px var(--font-mono, monospace); }
.editor-actions { align-items: center; display: flex; gap: 5px; }
.editor-icon { align-items: center; background: transparent; border: 1px solid var(--border); border-radius: 5px; color: var(--muted-foreground); cursor: pointer; display: inline-flex; height: 25px; justify-content: center; width: 25px; }
.editor-icon:hover { background: var(--surface-hover); color: var(--foreground); }
.editor-content { min-height: 0; flex: 1; overflow: auto; padding: 14px; }
.map-content { padding: 14px; }
.map-content :deep(.map-source-tool) { min-height: 100%; }
.asset-content { align-items: stretch; display: flex; flex-direction: column; gap: 0; overflow: hidden; padding: 0; }
.asset-content :deep(.asset-tool) { flex: 1; min-height: 0; width: 100%; }
.editor-description, .editor-empty { margin: 0; color: var(--muted-foreground); font-size: 12px; }
.editor-action { border: 1px solid var(--border); border-radius: 6px; background: var(--surface-raised, var(--surface)); color: var(--foreground); padding: 7px 10px; cursor: pointer; }
.default-asset-canvas { align-items: center; background: #0e1a31; border: 1px solid var(--border); border-radius: 8px; display: flex; flex: 1; justify-content: center; min-height: 260px; overflow: hidden; position: relative; width: 100%; }
.default-grid { background-image: linear-gradient(color-mix(in srgb, var(--primary) 20%, transparent) 1px, transparent 1px), linear-gradient(90deg, color-mix(in srgb, var(--primary) 20%, transparent) 1px, transparent 1px); background-size: 32px 32px; inset: 0; opacity: .7; position: absolute; transform: perspective(420px) rotateX(58deg) scale(1.5); transform-origin: center bottom; }
.default-canvas-copy { color: var(--muted-foreground); max-width: 300px; position: relative; text-align: center; }
.default-canvas-copy strong { color: var(--foreground); display: block; font-size: 13px; }
.default-canvas-copy p { font-size: 11px; line-height: 1.5; margin: 8px 0 0; }
.default-canvas-icon { color: var(--primary); display: block; font-size: 34px; margin-bottom: 8px; }
</style>
