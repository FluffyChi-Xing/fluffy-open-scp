<script setup lang="ts">
import { ref, toRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
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
const formatMenuOpen = ref(false);
const audioEl = ref<HTMLAudioElement | null>(null);
const playing = ref(false);
const currentTime = ref(0);
const duration = ref(0);
const seeking = ref(false);
function togglePlay() {
  const el = audioEl.value;
  if (!el) return;
  if (el.paused) void el.play();
  else el.pause();
}
function onTimeUpdate() {
  const el = audioEl.value;
  if (!el || seeking.value) return;
  currentTime.value = el.currentTime;
}
function seekTo(event: Event) {
  const el = audioEl.value;
  if (!el) return;
  const value = Number((event.target as HTMLInputElement).value);
  el.currentTime = value;
  currentTime.value = value;
}
function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds)) return "0:00";
  const m = Math.floor(seconds / 60);
  const sec = Math.floor(seconds % 60);
  return `${m}:${sec.toString().padStart(2, "0")}`;
}
const AUDIO_FORMATS: { value: ResourceExportFormat; label: string }[] = [
  { value: "wav", label: "WAV" },
  { value: "mp3", label: "MP3" },
  { value: "ogg", label: "OGG" },
  { value: "flac", label: "FLAC" },
];
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
      <FDropdown v-model:open="formatMenuOpen" :width="120">
        <template #trigger>
          <button
            class="format-button"
            type="button"
            :disabled="exporting"
            :aria-label="$t('package.exportFormat')"
          >
            <span class="format-label">{{ audioFormat.toUpperCase() }}</span>
            <FIcon name="ChevronDown" :size="11" aria-label="" />
          </button>
        </template>
        <button
          v-for="format in AUDIO_FORMATS"
          :key="format.value"
          type="button"
          :class="{ active: format.value === audioFormat }"
          @click="audioFormat = format.value; formatMenuOpen = false"
        >
          <FIcon
            :name="format.value === audioFormat ? 'Check' : 'FileMusic'"
            :size="13"
            aria-label=""
          />
          {{ format.label }}
        </button>
      </FDropdown>
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
      <div class="audio-player" role="group" :aria-label="$t('package.audioPreview')">
        <audio
          ref="audioEl"
          :src="preview.src"
          preload="metadata"
          @play="playing = true"
          @pause="playing = false"
          @ended="playing = false"
          @timeupdate="onTimeUpdate"
          @loadedmetadata="duration = audioEl?.duration ?? 0"
        />
        <button
          class="audio-play"
          type="button"
          :aria-label="playing ? $t('package.audioPause') : $t('package.audioPlay')"
          @click="togglePlay"
        >
          <FIcon :name="playing ? 'Pause' : 'Play'" :size="15" aria-label="" />
        </button>
        <span class="audio-time">{{ formatTime(currentTime) }}</span>
        <input
          class="audio-seek"
          type="range"
          min="0"
          :max="duration || 0"
          step="0.1"
          :value="currentTime"
          :aria-label="$t('package.audioSeek')"
          @input="seekTo"
        />
        <span class="audio-time">{{ formatTime(duration) }}</span>
      </div>
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
.format-button {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 5px;
  min-height: 28px;
  padding: 0 8px;
}
.format-button:hover,
.format-button:focus-visible {
  background: var(--accent);
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
.format-button:disabled {
  cursor: wait;
  opacity: 0.55;
}
.audio-player {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  display: flex;
  gap: 12px;
  padding: 10px 14px;
  width: min(560px, 100%);
}
.audio-player audio {
  display: none;
}
.audio-play {
  align-items: center;
  background: var(--accent);
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  height: 34px;
  justify-content: center;
  width: 34px;
  flex: none;
}
.audio-play:hover,
.audio-play:focus-visible {
  background: var(--primary);
  color: var(--primary-foreground, white);
  outline: none;
}
.audio-time {
  color: var(--muted-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  flex: none;
}
.audio-seek {
  accent-color: var(--primary);
  flex: 1;
  min-width: 80px;
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
