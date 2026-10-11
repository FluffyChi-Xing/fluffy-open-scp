<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { tauriApi } from "@/api";
import FSheet from "@/components/ui/FSheet.vue";
import RasterEditor from "@/components/raster/RasterEditor.vue";
import {
  RASTER_PROFILES,
  parseRasterSchema,
  validateParameters,
  type RasterProfile,
  type RasterSchema,
} from "@/lib/raster-editor/schema";
import { useRasterSession } from "@/components/raster/useRasterSession";
import { RasterDocument } from "@/lib/raster-editor/document";
import { base64ToRgba } from "@/lib/raster-editor/encoders";
import { type ProjectAsset } from "./assetFlow";
import type { FlowSchema } from "./contracts";
const props = defineProps<{
  project: string;
  schema: FlowSchema;
  busy: boolean;
}>();
const emit = defineEmits<{
  save: [schema: FlowSchema];
  dirty: [value: boolean];
  resize: [];
}>();
const open = ref(false),
  loading = ref(false),
  error = ref(""),
  draft = ref<RasterSchema>();
const localDirty = ref(false);
const slotIndex = computed(() => {
  const i = Number(props.schema.config.slot);
  return Number.isInteger(i) && i >= 0 && i <= 5 ? i : 0;
});
const slot = computed(() => RASTER_PROFILES[slotIndex.value]);
const initialProfile = computed(
  () => `slot${slotIndex.value}` as RasterProfile,
);
const session = useRasterSession();
let generation = 0;
watch(
  () => [
    props.project,
    props.schema.id,
    props.schema.config.document,
    props.schema.config.asset,
  ],
  () => {
    generation++;
    open.value = false;
    localDirty.value = false;
    emit("dirty", false);
  },
);
async function edit() {
  const epoch = ++generation;
  loading.value = true;
  error.value = "";
  try {
    if (props.schema.config.document) {
      draft.value = parseRasterSchema(String(props.schema.config.document));
      if (draft.value.profile !== initialProfile.value)
        throw new Error(t("rasterEditor.text163"));
    } else if (props.schema.config.asset) {
      const asset = await invoke<ProjectAsset>("code_flow", {
        request: {
          project: props.project,
          action: "read-asset",
          payload: { asset: props.schema.config.asset },
        },
      });
      if (epoch !== generation) return;
      session.changeProfile(initialProfile.value);
      session.name.value = slot.value.label;
      if (slotIndex.value === 0)
        session.parameters.value = validateParameters(
          JSON.parse(
            new TextDecoder().decode(
              Uint8Array.from(atob(asset.base64 ?? ""), (c) => c.charCodeAt(0)),
            ),
          ),
        );
      else {
        const image = await tauriApi.raster.readImageRgba(asset.path);
        if (epoch !== generation) return;
        session.install(
          new RasterDocument(
            image.width,
            image.height,
            base64ToRgba(image.rgbaBase64),
          ),
          { name: asset.asset },
          slot.value.label,
        );
      }
      draft.value = session.snapshot();
    } else draft.value = undefined;
    if (epoch === generation) open.value = true;
  } catch (e) {
    if (epoch === generation) error.value = String(e);
  } finally {
    if (epoch === generation) loading.value = false;
  }
}
function apply(value: RasterSchema) {
  emit("save", {
    ...props.schema,
    config: { ...props.schema.config, document: JSON.stringify(value) },
  });
  // Parent confirms persistence by replacing config; a failed save keeps the editor and dirty flag.
}
function close(value: boolean) {
  if (!value && localDirty.value && !window.confirm(t("rasterEditor.text164")))
    return;
  open.value = value;
  if (!value) {
    localDirty.value = false;
    emit("dirty", false);
  }
}
</script>
<template>
  <div class="raster-flow-tool nodrag nowheel nopan">
    <p>{{ slot.hint }}</p>
    <p>
      {{
        schema.config.document
          ? t("rasterEditor.text165")
          : schema.config.asset || t("rasterEditor.text166")
      }}
    </p>
    <button :disabled="busy || loading" @click="edit">
      {{ loading ? t("rasterEditor.text167") : t("rasterEditor.text168") }}
    </button>
    <p v-if="error" role="alert">{{ error }}</p>
    <FSheet
      :open="open"
      width="min(1440px, 96vw)"
      :label="t('rasterEditor.text169')"
      @update:open="close"
      ><RasterEditor
        v-if="open"
        :key="schema.id"
        mode="flow"
        :initial-schema="draft"
        :initial-profile="initialProfile"
        locked-profile
        :busy="busy"
        @schema="apply"
        @dirty="
          localDirty = $event;
          emit('dirty', $event);
        "
    /></FSheet>
  </div>
</template>
<style scoped>
.raster-flow-tool {
  display: grid;
  gap: 10px;
  padding: 12px;
}
p {
  font-size: 11px;
  color: var(--muted-foreground);
  margin: 0;
  overflow-wrap: anywhere;
}
button {
  padding: 8px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--foreground);
  border-radius: 5px;
  cursor: pointer;
}
[role="alert"] {
  color: var(--destructive);
}
</style>
