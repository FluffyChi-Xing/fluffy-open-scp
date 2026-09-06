<script setup lang="ts">
import { ref, toRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { AudioPreview as AudioPreviewData } from "@/api/tauri";
import {
  defaultAudioExportFormat,
  useResourceExport,
  type ResourceExportFormat,
} from "@/composables/useResourceExport";
import { useToast } from "@/composables/useToast";

const props = defineProps<{ preview: AudioPreviewData }>();
const { t } = useI18n();
const toast = useToast();
const audioFormat = ref<ResourceExportFormat>(defaultAudioExportFormat());
const { exporting, exportResource } = useResourceExport(toRef(props, "preview"));

watch(
  () => props.preview,
  (preview) => {
    if (!preview.toolAvailable)
      toast.error(t("package.audioToolMissing"), {
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
  <div class="media-preview audio-preview">
    <div class="media-toolbar" role="toolbar" :aria-label="$t('package.audioToolbar')">
      <FIcon name="Volume2" :size="15" aria-label="" />
      <span>{{ $t("package.audioPreview") }}</span>
      <span class="toolbar-spacer" />
      <span class="tool-status" :class="{ available: preview.toolAvailable }">
        {{ preview.toolAvailable ? preview.toolName : $t("package.toolMissing") }}
      </span>
      <select
        v-model="audioFormat"
        :aria-label="$t('package.exportFormat')"
        :disabled="exporting"
      >
        <option value="wav">WAV</option>
        <option value="mp3">MP3</option>
        <option value="ogg">OGG</option>
        <option value="flac">FLAC</option>
      </select>
      <button
        type="button"
        class="export-button"
        :disabled="exporting"
        :aria-busy="exporting"
        :aria-label="$t('package.exportResource')"
        @click="exportResource(audioFormat)"
      >
        {{ exporting ? $t("package.exporting") : $t("package.export") }}
      </button>
    </div>
    <div v-if="preview.src" class="media-viewport">
      <audio
        class="audio-player"
        :src="preview.src"
        controls
        preload="metadata"
        :aria-label="$t('package.audioPreview')"
      />
    </div>
    <div v-else class="media-viewport media-unavailable" role="alert">
      <FIcon name="VolumeX" :size="28" aria-label="" />
      <strong>{{ $t("package.audioToolMissing") }}</strong>
      <p>{{ $t("package.audioToolHint") }}</p>
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
.media-toolbar select,
.export-button {
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 11px;
  min-height: 28px;
  padding: 0 8px;
}
.export-button {
  cursor: pointer;
  font-weight: 650;
}
.export-button:hover,
.export-button:focus-visible,
.media-toolbar select:focus-visible {
  background: var(--accent);
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
.export-button:disabled,
.media-toolbar select:disabled {
  cursor: wait;
  opacity: 0.55;
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
  padding: 30px;
}
.audio-player {
  max-width: 100%;
  width: min(560px, 100%);
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
