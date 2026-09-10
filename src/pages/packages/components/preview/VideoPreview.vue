<script setup lang="ts">
import { ref, watch } from "vue";
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

const videoEl = ref<HTMLVideoElement | null>(null);
const playing = ref(false);
const currentTime = ref(0);
const duration = ref(0);
const muted = ref(false);
const seeking = ref(false);
function togglePlay() {
  const el = videoEl.value;
  if (!el) return;
  if (el.paused) void el.play();
  else el.pause();
}
function onTimeUpdate() {
  const el = videoEl.value;
  if (!el || seeking.value) return;
  currentTime.value = el.currentTime;
}
function seekTo(event: Event) {
  const el = videoEl.value;
  if (!el) return;
  const value = Number((event.target as HTMLInputElement).value);
  el.currentTime = value;
  currentTime.value = value;
}
function toggleMute() {
  const el = videoEl.value;
  if (!el) return;
  el.muted = !el.muted;
  muted.value = el.muted;
}
function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds)) return "0:00";
  const m = Math.floor(seconds / 60);
  const sec = Math.floor(seconds % 60);
  return `${m}:${sec.toString().padStart(2, "0")}`;
}

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
    <div v-if="preview.src" class="video-frame">
      <div class="media-viewport video-viewport">
        <video
          ref="videoEl"
          class="video-player"
          :src="preview.src"
          playsinline
          preload="metadata"
          :aria-label="$t('package.videoPreview')"
          @click="togglePlay"
          @play="playing = true"
          @pause="playing = false"
          @ended="playing = false"
          @timeupdate="onTimeUpdate"
          @loadedmetadata="duration = videoEl?.duration ?? 0"
        />
      </div>
      <div class="video-controls" role="group" :aria-label="$t('package.videoToolbar')">
        <button
          class="video-button"
          type="button"
          :aria-label="playing ? $t('package.audioPause') : $t('package.audioPlay')"
          @click="togglePlay"
        >
          <FIcon :name="playing ? 'Pause' : 'Play'" :size="15" aria-label="" />
        </button>
        <span class="video-time">{{ formatTime(currentTime) }}</span>
        <input
          class="video-seek"
          type="range"
          min="0"
          :max="duration || 0"
          step="0.1"
          :value="currentTime"
          :aria-label="$t('package.audioSeek')"
          @input="seekTo"
        />
        <span class="video-time">{{ formatTime(duration) }}</span>
        <button
          class="video-button"
          type="button"
          :aria-label="$t('package.videoMute')"
          @click="toggleMute"
        >
          <FIcon :name="muted ? 'VolumeX' : 'Volume2'" :size="15" aria-label="" />
        </button>
      </div>
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
.video-frame {
  display: grid;
  gap: 8px;
  min-width: 0;
}
.video-controls {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  display: flex;
  gap: 12px;
  padding: 8px 12px;
}
.video-button {
  align-items: center;
  background: var(--accent);
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  height: 30px;
  justify-content: center;
  width: 30px;
  flex: none;
}
.video-button:hover,
.video-button:focus-visible {
  background: var(--primary);
  color: var(--primary-foreground, white);
  outline: none;
}
.video-time {
  color: var(--muted-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  flex: none;
}
.video-seek {
  accent-color: var(--primary);
  flex: 1;
  min-width: 80px;
}
.media-meta {
  color: var(--muted-foreground);
  display: flex;
  flex-wrap: wrap;
  font-size: 11px;
  gap: 10px;
}
</style>
