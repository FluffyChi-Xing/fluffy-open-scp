<script setup lang="ts">
import { computed, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FSheet from "@/components/ui/FSheet.vue";
import { createDataSource } from "@/api/data-source";
import type { ImagePreview, Rw4Preview, Rw4SectionDetail, Rw4Section } from "@/api/tauri";
import MeshPreviewView from "./MeshPreview.vue";
import { exportLotModel } from "@/composables/useModelExport";
import ImagePreviewView from "./ImagePreview.vue";

const props = defineProps<{ preview: Rw4Preview }>();
const { t } = useI18n();
const source = createDataSource();
const open = ref(false);
const loading = ref(false);
const error = ref("");
const detail = shallowRef<Rw4SectionDetail | null>(null);
const meshExportBusy = ref(false);
async function exportMesh() {
  if (meshExportBusy.value) return;
  meshExportBusy.value = true;
  try {
    await exportLotModel({
      packageId: props.preview.packageId,
      modelTgi: props.preview.tgi,
      mode: "white",
      defaultName: `0x${props.preview.tgi.instance.toString(16).padStart(8, "0")}`,
    });
  } finally {
    meshExportBusy.value = false;
  }
}

function typeLabel(section: Rw4Section) {
  return section.typeName ?? `0x${section.typeCode.toString(16).toUpperCase()}`;
}
async function openSection(section: Rw4Section) {
  open.value = true;
  loading.value = true;
  error.value = "";
  try {
    detail.value = await source.readRw4Section(
      props.preview.packageId,
      props.preview.tgi,
      section.number,
    );
  } catch {
    error.value = t("package.detailLoadFailed");
  } finally {
    loading.value = false;
  }
}
function boundsLabel(
  bounds: [number, number, number] | null,
  boundsMax: [number, number, number] | null,
) {
  if (!bounds || !boundsMax) return "—";
  const format = (value: [number, number, number]) =>
    `(${value.map((axis) => axis.toFixed(2)).join(", ")})`;
  return `${format(bounds)} – ${format(boundsMax)}`;
}
const textureAsImage = computed<ImagePreview | null>(() => {
  const texture = detail.value?.texture;
  if (!texture) return null;
  return {
    kind: "image",
    offset: 0,
    totalLength: 0,
    bytes: [],
    src: `data:image/png;base64,${texture.pngBase64}`,
    mime: "image/png",
    width: texture.width,
    height: texture.height,
  };
});
</script>

<template>
  <div class="structured-preview">
    <div class="rw4-meta">
      <span>{{ preview.fileType }}</span>
      <span>{{ preview.sections.length }} sections</span>
    </div>
    <div class="structured-table-wrap">
      <table class="structured-table">
        <thead>
          <tr>
            <th>{{ $t("package.numberColumn") }}</th>
            <th>{{ $t("package.typeCodeColumn") }}</th>
            <th>{{ $t("package.sectionSize") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="section in preview.sections"
            :key="section.number"
            class="section-row"
            @click="openSection(section)"
          >
            <td class="section-number">#{{ section.number }}</td>
            <td>
              <span class="section-type">{{ typeLabel(section) }}</span>
              <small v-if="section.typeName">{{
                `0x${section.typeCode.toString(16).toUpperCase()}`
              }}</small>
            </td>
            <td class="section-size">{{ section.size.toLocaleString() }} B</td>
          </tr>
        </tbody>
      </table>
      <p v-if="!preview.sections.length" class="empty-hint">
        {{ $t("package.noPreview") }}
      </p>
    </div>

    <FSheet
      v-model:open="open"
      :label="$t('package.sectionDetail')"
      width="clamp(420px, 38vw, 760px)"
    >
      <header class="sheet-header">
        <div>
          <strong v-if="detail">{{
            detail.typeName ?? `0x${detail.typeCode.toString(16).toUpperCase()}`
          }}</strong>
          <small v-if="detail"
            >#{{ detail.number }} · {{ $t("package.sectionOffset") }}
            {{ detail.pos.toLocaleString() }} ·
            {{ $t("package.sectionSize") }}
            {{ detail.size.toLocaleString() }} B</small
          >
        </div>
        <button
          class="sheet-close"
          type="button"
          :aria-label="$t('shell.close')"
          @click="open = false"
        >
          <FIcon name="X" :size="15" aria-label="" />
        </button>
      </header>
      <div v-if="loading" class="sheet-loading">
        <FSpinner size="sm" :label="$t('common.loading')" />
      </div>
      <p v-else-if="error" class="sheet-error" role="alert">{{ error }}</p>
      <div v-else-if="detail" class="sheet-body">
        <template v-if="detail.mesh">
          <div class="preview-toolbar" role="toolbar" aria-label="Mesh toolbar">
            <button type="button" disabled>
              <FIcon name="Download" :size="13" aria-label="" />{{
                $t("package.toolbarImport")
              }}
            </button>
            <button
              type="button"
              :disabled="meshExportBusy"
              @click="exportMesh"
            >
              <FIcon :name="meshExportBusy ? 'Loader2' : 'Upload'" :size="13" aria-label="" />{{
                $t("package.toolbarExport")
              }}
            </button>
            <button type="button" disabled>
              <FIcon name="Image" :size="13" aria-label="" />{{
                $t("package.toolbarTexture")
              }}
            </button>
            <button type="button" disabled>
              <FIcon name="Wrench" :size="13" aria-label="" />{{
                $t("package.toolbarMeshTools")
              }}
              <FIcon name="ChevronDown" :size="11" aria-label="" />
            </button>
          </div>
          <dl class="detail-grid">
            <div>
              <dt>{{ $t("package.meshTriangles") }}</dt>
              <dd>{{ detail.mesh.decodedTriangles.toLocaleString() }}</dd>
            </div>
            <div>
              <dt>{{ $t("package.meshVertices") }}</dt>
              <dd>{{ detail.mesh.decodedVertices.toLocaleString() }}</dd>
            </div>
            <div>
              <dt>{{ $t("package.meshBounds") }}</dt>
              <dd>
                {{
                  boundsLabel(detail.mesh.boundsMin, detail.mesh.boundsMax)
                }}
              </dd>
            </div>
            <div>
              <dt>{{ $t("package.meshExportable") }}</dt>
              <dd>
                {{ detail.mesh.exportable ? $t("package.yes") : $t("package.no") }}
              </dd>
            </div>
          </dl>
          <MeshPreviewView
            v-if="detail.mesh.objBase64"
            :obj-base64="detail.mesh.objBase64"
          />
        </template>
        <template v-else-if="textureAsImage">
          <ImagePreviewView :preview="textureAsImage" />
          <dl class="detail-grid">
            <div>
              <dt>{{ $t("package.sectionSize") }}</dt>
              <dd>{{ textureAsImage.width }} × {{ textureAsImage.height }}</dd>
            </div>
            <div>
              <dt>Mip</dt>
              <dd>{{ detail.texture?.mipCount ?? "—" }}</dd>
            </div>
          </dl>
        </template>
        <template v-else-if="detail.hexDump">
          <p class="dump-label">{{ $t("package.rawBytes") }}</p>
          <pre class="hex-dump">{{ detail.hexDump }}</pre>
        </template>
      </div>
    </FSheet>
  </div>
</template>

<style scoped>
.structured-preview {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
}
.rw4-meta {
  color: var(--subtle-foreground);
  display: flex;
  font-size: 11px;
  gap: 10px;
}
.structured-table-wrap {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  max-height: 420px;
  min-height: 240px;
  overflow: auto;
}
.structured-table {
  border-collapse: collapse;
  font-size: 12px;
  min-width: 100%;
  text-align: start;
}
.structured-table th {
  background: var(--surface-elevated);
  border-bottom: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  padding: 9px 10px;
  position: sticky;
  text-align: start;
  text-transform: uppercase;
  top: 0;
}
.structured-table td {
  border-top: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
  padding: 8px 10px;
}
.section-row {
  cursor: pointer;
}
.section-row:hover {
  background: var(--accent);
}
.section-number {
  color: var(--primary);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  min-width: 56px;
}
.section-type {
  color: var(--foreground);
  font-weight: 600;
}
.section-type + small {
  color: var(--subtle-foreground);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 10px;
  margin-inline-start: 6px;
}
.section-size {
  color: var(--muted-foreground);
}
.empty-hint {
  color: var(--subtle-foreground);
  font-size: 12px;
  padding: 26px 12px;
  text-align: center;
}
.sheet-header {
  align-items: flex-start;
  border-bottom: 1px solid var(--border);
  display: flex;
  justify-content: space-between;
  padding: 16px;
  position: sticky;
  top: 0;
  background: var(--surface);
  z-index: 1;
}
.sheet-header strong {
  display: block;
  font-size: 14px;
}
.sheet-header small {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.sheet-close {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  min-height: 28px;
  min-width: 28px;
  justify-content: center;
}
.sheet-close:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.sheet-loading {
  display: grid;
  justify-items: center;
  padding: 40px;
}
.sheet-error {
  color: var(--danger);
  font-size: 12px;
  padding: 20px 16px;
}
.sheet-body {
  display: grid;
  gap: 12px;
  min-width: 0;
  padding: 14px 16px 16px;
}
.dump-label {
  color: var(--subtle-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  margin: 0;
  text-transform: uppercase;
}
.preview-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.preview-toolbar button {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: not-allowed;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 5px;
  min-height: 28px;
  opacity: 0.6;
  padding: 0 9px;
}
.detail-grid {
  display: grid;
  gap: 10px;
  grid-template-columns: 1fr 1fr;
  margin: 0;
}
.detail-grid dt {
  color: var(--subtle-foreground);
  font-size: 10px;
  margin-bottom: 3px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
.detail-grid dd {
  color: var(--foreground);
  font-size: 12px;
  margin: 0;
  overflow-wrap: anywhere;
}
.hex-dump {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  font: 11px/1.7 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  margin: 0;
  overflow-x: auto;
  padding: 12px;
  white-space: pre;
}
</style>
