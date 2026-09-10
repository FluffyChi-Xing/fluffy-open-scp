<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import type { FontPreview } from "@/api/tauri";

const props = defineProps<{ preview: FontPreview }>();

const status = ref<"loading" | "ready" | "error">("loading");
const family = ref("");
let loadedFace: FontFace | null = null;

const SAMPLE_LARGE = "SimCity 2013";
const SAMPLE_SMALL =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789";
const SAMPLE_CJK = "城市模拟 建造道路 电力供水 幸福度";

async function loadFont(src: string) {
  status.value = "loading";
  const face = new FontFace(`openscp-preview-${Date.now()}`, `url(${src})`);
  try {
    await face.load();
  } catch {
    status.value = "error";
    return;
  }
  document.fonts.add(face);
  loadedFace = face;
  family.value = face.family;
  status.value = "ready";
}

watch(
  () => props.preview.src,
  (src) => {
    void loadFont(src);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (loadedFace) document.fonts.delete(loadedFace);
});
</script>

<template>
  <div class="font-preview">
    <div class="font-toolbar" role="toolbar" :aria-label="$t('package.fontPreview')">
      <FIcon name="Type" :size="15" aria-label="" />
      <span>{{ $t("package.fontPreview") }}</span>
      <span class="toolbar-spacer" />
      <span class="font-meta">{{ preview.totalLength.toLocaleString() }} B</span>
    </div>
    <div v-if="status === 'loading'" class="font-stage font-loading">
      {{ $t("common.loading") }}
    </div>
    <div v-else-if="status === 'error'" class="font-stage font-error" role="alert">
      <FIcon name="FileQuestion" :size="26" aria-label="" />
      <FTypography :header="4" spacing="none">{{
        $t("package.fontPreviewFailed")
      }}</FTypography>
    </div>
    <div v-else class="font-stage" :style="{ fontFamily: `'${family}'` }">
      <p class="font-sample font-sample-xl">{{ SAMPLE_LARGE }}</p>
      <p class="font-sample">{{ SAMPLE_SMALL }}</p>
      <p class="font-sample font-sample-sm">{{ SAMPLE_SMALL }}</p>
      <p class="font-sample font-sample-sm font-fallback-cjk">{{ SAMPLE_CJK }}</p>
    </div>
  </div>
</template>

<style scoped>
.font-preview {
  display: grid;
  gap: 10px;
  min-width: 0;
}
.font-toolbar {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  display: flex;
  font-size: 12px;
  gap: 7px;
  min-height: 38px;
  padding: 5px 9px;
}
.toolbar-spacer {
  flex: 1;
}
.font-meta {
  color: var(--muted-foreground);
  font-size: 11px;
}
.font-stage {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  display: grid;
  gap: 14px;
  min-height: 220px;
  align-content: center;
  padding: 28px;
}
.font-loading,
.font-error {
  color: var(--muted-foreground);
  font-size: 12px;
  justify-items: center;
}
.font-sample {
  margin: 0;
  overflow-wrap: anywhere;
  font-size: 15px;
  line-height: 1.6;
}
.font-sample-xl {
  font-size: 34px;
  font-weight: 600;
  line-height: 1.25;
}
.font-sample-sm {
  font-size: 12px;
  color: var(--muted-foreground);
}
.font-fallback-cjk {
  font-family: var(--font-sans, sans-serif);
}
</style>
