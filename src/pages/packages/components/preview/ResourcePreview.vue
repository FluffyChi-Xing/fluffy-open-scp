<script setup lang="ts">
import { computed } from "vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import type {
  ResourcePreview,
  TextPreview,
  HexPreview,
  ImagePreview,
} from "@/api/tauri";
import TextPreviewView from "./TextPreview.vue";
import HexPreviewView from "./HexPreview.vue";
import ImagePreviewView from "./ImagePreview.vue";

interface Props {
  preview: ResourcePreview | null;
  loading?: boolean;
  error?: string;
}
const props = withDefaults(defineProps<Props>(), { loading: false, error: "" });
const textPreview = computed(() =>
  props.preview?.kind === "text" ? (props.preview as TextPreview) : null,
);
const hexPreview = computed(() =>
  props.preview?.kind === "hex" ? (props.preview as HexPreview) : null,
);
const imagePreview = computed(() =>
  props.preview?.kind === "image" ? (props.preview as ImagePreview) : null,
);
</script>

<template>
  <section class="resource-preview" aria-live="polite">
    <div v-if="props.loading" class="preview-loading">
      <FSkeleton height="18px" width="42%" rounded />
      <FSkeleton height="12px" width="72%" />
      <FSkeleton height="12px" width="58%" />
      <FSpinner size="sm" :label="$t('common.loading')" />
    </div>
    <p v-else-if="props.error" class="preview-error" role="alert">
      {{ props.error }}
    </p>
    <TextPreviewView v-else-if="textPreview" :preview="textPreview" />
    <HexPreviewView v-else-if="hexPreview" :preview="hexPreview" />
    <ImagePreviewView v-else-if="imagePreview" :preview="imagePreview" />
    <div
      v-else-if="props.preview?.kind === 'unsupported'"
      class="preview-empty"
    >
      <FTypography :header="4" spacing="none">{{
        $t("package.unsupportedPreview")
      }}</FTypography>
      <p>{{ props.preview.reason }}</p>
    </div>
    <div v-else class="preview-empty">
      <FTypography :header="4" spacing="none">{{
        $t("package.noPreview")
      }}</FTypography>
    </div>
  </section>
</template>

<style scoped>
.resource-preview {
  min-height: 300px;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.preview-loading {
  display: grid;
  align-content: center;
  justify-items: center;
  gap: 16px;
  min-height: 300px;
  padding: 24px;
}
.preview-error {
  color: var(--danger);
  font-size: 12px;
  margin: auto;
  padding: 24px;
}
.preview-empty {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
  min-height: 300px;
  padding: 24px;
  text-align: center;
}
.preview-empty p {
  font-size: 12px;
  line-height: 1.5;
  margin: 0;
  max-width: 340px;
}
</style>
