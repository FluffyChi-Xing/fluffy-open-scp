<script setup lang="ts">
import { computed, defineAsyncComponent, ref } from "vue";
import { useI18n } from "vue-i18n";
import MapSourceTool from "./mod-flow/MapSourceTool.vue";
import type { Tgi } from "@/api/tauri";

const PropertyEditor = defineAsyncComponent(
  () => import("@/pages/packages/components/property-editor/PropertyEditor.vue"),
);

const props = defineProps<{
  project: string;
  kind: string;
  manifest: Record<string, unknown>;
}>();
const { t } = useI18n();
const propertyOpen = ref(false);
const mapPackagePath = ref("");
const mapGroup = ref("");

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
  <section class="standard-editor" :aria-label="editorTitle">
    <header class="standard-editor-head">
      <div>
        <p class="eyebrow">{{ t("studio.code.standardWorkbench") }}</p>
        <h2>{{ editorTitle }}</h2>
      </div>
      <span class="editor-kind">{{ kind }}</span>
    </header>

    <div v-if="kind === 'map'" class="editor-content map-content">
      <MapSourceTool v-model:package-path="mapPackagePath" v-model:group="mapGroup" :disabled="false" />
    </div>
    <div v-else-if="kind === 'assets'" class="editor-content asset-content">
      <p v-if="assetTarget" class="editor-description">{{ t("studio.code.assetEditorHint") }}</p>
      <button v-if="assetTarget" type="button" class="editor-action" @click="propertyOpen = true">
        {{ t("studio.code.openAssetEditor") }}
      </button>
      <p v-else class="editor-empty">{{ t("studio.code.assetEditorUnavailable") }}</p>
      <PropertyEditor
        v-if="assetTarget"
        v-model:open="propertyOpen"
        :package-id="assetTarget.packageId"
        :tgi="assetTarget.tgi"
        mode="editor"
      />
    </div>
    <div v-else class="editor-content editor-empty">
      {{ t("studio.code.editorPending") }}
    </div>
  </section>
</template>

<style scoped>
.standard-editor { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--surface); }
.standard-editor-head { display: flex; align-items: center; justify-content: space-between; padding: 12px 14px; border-bottom: 1px solid var(--border); flex: 0 0 auto; }
.standard-editor-head h2 { margin: 2px 0 0; font-size: 14px; font-weight: 600; }
.eyebrow { margin: 0; color: var(--muted-foreground); font-size: 10px; text-transform: uppercase; letter-spacing: .08em; }
.editor-kind { color: var(--muted-foreground); font: 11px var(--font-mono, monospace); }
.editor-content { min-height: 0; flex: 1; overflow: auto; padding: 14px; }
.map-content { padding: 14px; }
.map-content :deep(.map-source-tool) { min-height: 100%; }
.asset-content { display: flex; flex-direction: column; align-items: flex-start; gap: 10px; }
.editor-description, .editor-empty { margin: 0; color: var(--muted-foreground); font-size: 12px; }
.editor-action { border: 1px solid var(--border); border-radius: 6px; background: var(--surface-raised, var(--surface)); color: var(--foreground); padding: 7px 10px; cursor: pointer; }
</style>
