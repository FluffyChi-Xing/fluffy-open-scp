<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { isTauri } from "@/api";
import FIcon from "@/components/extensions/FIcon.vue";
import FSheet from "@/components/ui/FSheet.vue";
import RasterEditor from "@/components/raster/RasterEditor.vue";
import RasterLotPreview from "@/components/raster/RasterLotPreview.vue";
import {
  authoredAlbedoDataUrl,
  authoredFromSchema,
  maskPngDataUrl,
  schemaFromAuthored,
  type AuthoredLot,
} from "@/lib/lot-ground/authored";
import type { RasterSchema } from "@/lib/raster-editor/schema";
import { loadAuthoredLot, saveAuthoredLot } from "./authoredLotStore";

/**
 * Lot 库顶部的自绘地面区：第 1 格"绘制"（空 3D 容器，点开全屏绘制 sheet），
 * 第 2 格为项目内已保存的自绘地面预览（点击重新编辑；已绑定时右上角标记）。
 * 关闭 sheet 即保存到 assets/lot/authored/。
 */
const props = defineProps<{
  project: string;
  editEnabled: boolean;
  editMode: "native" | "draft";
  bound: boolean;
}>();
const emit = defineEmits<{
  apply: [lot: AuthoredLot];
  saved: [lot: AuthoredLot];
}>();
const { t } = useI18n();

const authored = shallowRef<AuthoredLot | null>(null);
const sheetOpen = ref(false);
const sheetKey = ref(0);
const initialSchema = shallowRef<RasterSchema | undefined>(undefined);
const editor = ref<InstanceType<typeof RasterEditor> | null>(null);
const editorDirty = ref(false);
const saving = ref(false);
const error = ref("");

onMounted(async () => {
  if (isTauri()) authored.value = await loadAuthoredLot(props.project);
});

const preview = computed(() => {
  const lot = authored.value;
  if (!lot) return null;
  return {
    lot,
    png: maskPngDataUrl(lot),
    albedo: authoredAlbedoDataUrl(lot),
    material: schemaFromAuthored(lot).lot,
  };
});
const canApply = computed(() => props.editEnabled && props.editMode === "draft");

function openSheet(schema: RasterSchema | undefined) {
  initialSchema.value = schema;
  editorDirty.value = false;
  error.value = "";
  sheetKey.value += 1;
  sheetOpen.value = true;
}

function startDrawing() {
  if (authored.value && !window.confirm(t("rasterEditor.ground.workbench.replaceConfirm"))) return;
  openSheet(undefined);
}

function editAuthored() {
  if (authored.value) openSheet(schemaFromAuthored(authored.value));
}

/** 关闭或点"完成绘制"：有改动就保存；保存失败则保持 sheet 打开，避免丢稿。 */
async function finish(schema?: RasterSchema) {
  if (saving.value) return;
  const value = schema ?? (editorDirty.value ? editor.value?.snapshot() : null);
  if (value?.image) {
    saving.value = true;
    error.value = "";
    try {
      const lot = authoredFromSchema(value);
      await saveAuthoredLot(props.project, lot);
      authored.value = lot;
      emit("saved", lot);
    } catch (cause) {
      error.value = t("rasterEditor.ground.workbench.saveFailed", { error: String(cause) });
      return;
    } finally {
      saving.value = false;
    }
  }
  sheetOpen.value = false;
}

function onSheetOpen(open: boolean) {
  if (!open) void finish();
}

function onStageKey(event: KeyboardEvent, action: () => void) {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    action();
  }
}
</script>

<template>
  <section class="authored-section" :aria-label="t('rasterEditor.ground.workbench.section')">
    <div class="authored-cards">
      <div class="lot-card">
        <div
          class="authored-stage draw-stage"
          role="button"
          tabindex="0"
          :title="t('rasterEditor.ground.workbench.drawHint')"
          @click="startDrawing"
          @keydown="onStageKey($event, startDrawing)"
        >
          <FIcon name="Pencil" :size="18" aria-label="" />
          <strong>{{ t("rasterEditor.ground.workbench.draw") }}</strong>
          <span>{{ t("rasterEditor.ground.workbench.drawHint") }}</span>
        </div>
      </div>
      <div v-if="preview" class="lot-card">
        <div
          class="authored-stage"
          role="button"
          tabindex="0"
          :title="t('rasterEditor.ground.workbench.edit')"
          :aria-label="t('rasterEditor.ground.workbench.edit')"
          @click="editAuthored"
          @keydown="onStageKey($event, editAuthored)"
        >
          <RasterLotPreview
            v-if="!sheetOpen"
            class="authored-preview"
            :raw="preview.lot.mask"
            :png="preview.png"
            :width="preview.lot.meta.width"
            :height="preview.lot.meta.height"
            :meters-per-pixel="preview.lot.meta.metersPerPixel"
            :colors="preview.lot.meta.colors"
            :target="null"
            :material="preview.material"
            :texture="false"
            :albedo-png="preview.albedo"
          />
          <span
            v-if="bound"
            class="bound-tag"
            :title="t('rasterEditor.ground.workbench.boundHint')"
          >
            <FIcon name="Link" :size="10" aria-label="" />
            {{ t("rasterEditor.ground.workbench.bound") }}
          </span>
        </div>
        <div class="lot-card-meta">
          <span class="lot-card-name">{{ t("rasterEditor.ground.workbench.authoredName") }}</span>
          <span class="lot-card-size">
            {{ preview.lot.meta.size[0] }} × {{ preview.lot.meta.size[1] }} m
          </span>
          <button
            type="button"
            class="lot-apply"
            :class="{ applied: bound }"
            :disabled="!canApply"
            :title="
              editMode !== 'draft'
                ? t('studio.assetWorkbench.lotApplyNativeHint')
                : t('studio.assetWorkbench.lotApplyDraftHint')
            "
            @click="emit('apply', preview.lot)"
          >
            <FIcon :name="bound ? 'Check' : 'Download'" :size="12" aria-label="" />
            {{ bound ? t("studio.assetWorkbench.lotApplied") : t("studio.assetWorkbench.lotApply") }}
          </button>
        </div>
      </div>
    </div>

    <FSheet
      :open="sheetOpen"
      :label="t('rasterEditor.ground.workbench.sheetTitle')"
      :close-on-escape="false"
      width="100vw"
      @update:open="onSheetOpen"
    >
      <div class="sheet-body">
        <header class="sheet-head">
          <strong>{{ t("rasterEditor.ground.workbench.sheetTitle") }}</strong>
          <p v-if="error" class="sheet-error" role="alert">{{ error }}</p>
          <button type="button" class="sheet-close" :disabled="saving" @click="finish()">
            <FIcon :name="saving ? 'Loader2' : 'X'" :size="14" aria-label="" />
            {{ saving ? t("rasterEditor.ground.workbench.saving") : t("rasterEditor.ground.workbench.close") }}
          </button>
        </header>
        <RasterEditor
          v-if="sheetOpen"
          :key="sheetKey"
          ref="editor"
          class="sheet-editor"
          mode="flow"
          initial-profile="lot-mask"
          locked-profile
          :initial-schema="initialSchema"
          :apply-label="t('rasterEditor.ground.workbench.finish')"
          :busy="saving"
          @dirty="editorDirty = $event"
          @schema="finish"
        />
      </div>
    </FSheet>
  </section>
</template>

<style scoped>
.authored-section { flex: none; }
.authored-cards { display: grid; gap: 8px; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.lot-card { border: 1px solid var(--border); border-radius: var(--radius-sm); display: flex; flex-direction: column; gap: 5px; min-width: 0; padding: 5px; }
.authored-stage { background: #10192b; border: 1px solid var(--border); border-radius: var(--radius-sm); cursor: pointer; display: flex; flex-direction: column; height: 118px; overflow: hidden; position: relative; }
.authored-stage:hover, .authored-stage:focus-visible { border-color: var(--primary); outline: none; }
.authored-preview { pointer-events: none; }
.draw-stage { align-items: center; color: var(--muted-foreground); gap: 4px; justify-content: center; text-align: center; }
.draw-stage strong { color: var(--foreground); font-size: 12px; }
.draw-stage span { font-size: 10px; padding: 0 6px; }
.draw-stage:hover { color: var(--primary); }
.bound-tag { align-items: center; background: color-mix(in srgb, var(--success) 85%, black); border-radius: 999px; color: white; display: inline-flex; font-size: 10px; gap: 3px; padding: 1px 7px; position: absolute; right: 5px; top: 5px; z-index: 1; }
.lot-card-meta { align-items: center; display: flex; gap: 5px; }
.lot-card-name { color: var(--foreground); flex: 1; font-size: 10.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.lot-card-size { color: var(--muted-foreground); flex: none; font-size: 10px; font-variant-numeric: tabular-nums; }
.lot-apply { align-items: center; background: var(--surface-elevated); border: 1px solid var(--border); border-radius: var(--radius-sm); color: var(--foreground); cursor: pointer; display: inline-flex; flex: none; font: inherit; font-size: 10px; gap: 3px; min-height: 22px; padding: 0 6px; white-space: nowrap; }
.lot-apply:hover:not(:disabled) { border-color: var(--primary); color: var(--primary); }
.lot-apply:disabled { cursor: not-allowed; opacity: 0.5; }
.lot-apply.applied { border-color: var(--success); color: var(--success); }
.sheet-body { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.sheet-head { align-items: center; border-bottom: 1px solid var(--border); display: flex; flex: none; gap: 12px; padding: 8px 16px; }
.sheet-head strong { font-size: 13px; }
.sheet-error { color: var(--danger); flex: 1; font-size: 11px; margin: 0; overflow-wrap: anywhere; }
.sheet-close { align-items: center; background: var(--surface-elevated); border: 1px solid var(--border); border-radius: var(--radius-sm); color: var(--foreground); cursor: pointer; display: inline-flex; font: inherit; font-size: 12px; gap: 5px; margin-left: auto; padding: 5px 10px; }
.sheet-close:disabled { opacity: 0.6; }
.sheet-editor { flex: 1; min-height: 0; }
</style>
