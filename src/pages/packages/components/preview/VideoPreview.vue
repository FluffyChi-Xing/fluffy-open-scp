<script setup lang="ts">
import { watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { VideoPreview as VideoPreviewData } from "@/api/tauri";
import { useToast } from "@/composables/useToast";

const props = defineProps<{ preview: VideoPreviewData }>();
const { t } = useI18n();
const toast = useToast();

watch(
  () => props.preview,
  (preview) => {
    if (!preview.toolAvailable)
      toast.error(t("package.videoToolMissing"), {
        title: t("package.mediaToolUnavailable"),
      });
  },
  { immediate: true },
);

async function copyInstallCommand() {
  try {
    await navigator.clipboard.writeText(props.preview.installCommand);
    toast.success(t("package.commandCopied"));
  } catch {
    toast.error(t("package.commandCopyFailed"));
  }
}
</script>

<template>
  <div class="media-preview video-preview">
    <div class="media-toolbar" role="toolbar" :aria-label="$t('package.videoToolbar')">
      <FIcon name="Film" :size="15" aria-label="" />
      <span>{{ $t("package.videoPreview") }}</span>
      <span class="toolbar-spacer" />
      <span class="tool-status" :class="{ available: preview.toolAvailable }">
        {{ preview.toolAvailable ? preview.toolName : $t("package.toolMissing") }}
      </span>
    </div>
    <div v-if="preview.src" class="media-viewport video-viewport">
      <video
        class="video-player"
        :src="preview.src"
        controls
        playsinline
        preload="metadata"
        :aria-label="$t('package.videoPreview')"
      />
    </div>
    <div v-else class="media-viewport media-unavailable" role="alert">
      <FIcon name="Film" :size="28" aria-label="" />
      <strong>{{ $t("package.videoToolMissing") }}</strong>
      <p>{{ $t("package.videoToolHint") }}</p>
      <code>{{ preview.installCommand }}</code>
      <button type="button" class="install-copy" @click="copyInstallCommand">
        <FIcon name="Copy" :size="13" aria-label="" />
        {{ $t("package.copyInstallCommand") }}
      </button>
    </div>
    <div class="media-meta">
      <span>{{ preview.mime }}</span>
      <span>{{ preview.totalLength.toLocaleString() }} B</span>
      <span v-if="preview.outputBytes">· {{ preview.outputBytes.toLocaleString() }} B</span>
    </div>
  </div>
</template>

<style scoped>
.media-preview {
  display: grid;
  gap: 10px;
  min-width: 0;
}
.media-toolbar {
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
.tool-status {
  color: var(--warning);
  font-size: 11px;
}
.tool-status.available {
  color: var(--success);
}
.media-viewport {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  justify-content: center;
  min-height: 220px;
  overflow: hidden;
  padding: 20px;
}
.video-viewport {
  background: #090b0f;
  min-height: 300px;
}
.video-player {
  display: block;
  max-height: 420px;
  max-width: 100%;
  width: 100%;
}
.media-unavailable {
  color: var(--muted-foreground);
  flex-direction: column;
  gap: 10px;
  padding: 34px 24px;
  text-align: center;
}
.media-unavailable strong {
  color: var(--foreground);
  font-size: 14px;
}
.media-unavailable p {
  font-size: 12px;
  line-height: 1.5;
  margin: 0;
  max-width: 420px;
}
.media-unavailable code {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: 11px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  max-width: 100%;
  overflow: auto;
  padding: 9px 10px;
  user-select: all;
}
.install-copy {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 6px;
  min-height: 30px;
  padding: 0 10px;
}
.install-copy:hover,
.install-copy:focus-visible {
  background: var(--accent);
}
.media-meta {
  color: var(--muted-foreground);
  display: flex;
  flex-wrap: wrap;
  font-size: 11px;
  gap: 10px;
}
</style>
