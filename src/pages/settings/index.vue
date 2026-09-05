<script setup lang="ts">
import { onMounted, shallowRef } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import {
  isTauri,
  tauriApi,
  type GameDirectoryDetection,
  type SettingsStatus,
  type WorkspaceStatus,
} from "@/api";

const gamePath = shallowRef("");
const status = shallowRef<SettingsStatus | null>(null);
const workspace = shallowRef<WorkspaceStatus | null>(null);
const detection = shallowRef<GameDirectoryDetection | null>(null);
const loading = shallowRef(false);
const saved = shallowRef(false);
const error = shallowRef("");

onMounted(() => {
  if (isTauri()) load();
});
async function load() {
  loading.value = true;
  error.value = "";
  try {
    [status.value, workspace.value, detection.value] = await Promise.all([
      tauriApi.settings.get(),
      tauriApi.workspace.get(),
      tauriApi.settings.detectGameDirectory(),
    ]);
    gamePath.value = status.value.gameDataPath ?? "";
  } catch (cause) {
    error.value = messageOf(cause);
  } finally {
    loading.value = false;
  }
}
async function saveGameDirectory() {
  if (!gamePath.value.trim()) return;
  loading.value = true;
  error.value = "";
  saved.value = false;
  try {
    status.value = await tauriApi.settings.setGameDirectory(
      gamePath.value.trim(),
    );
    saved.value = true;
    detection.value = await tauriApi.settings.detectGameDirectory();
  } catch (cause) {
    error.value = messageOf(cause);
  } finally {
    loading.value = false;
  }
}
function messageOf(cause: unknown) {
  if (cause && typeof cause === "object" && "message" in cause)
    return String(cause.message);
  return cause instanceof Error ? cause.message : String(cause);
}
</script>

<template>
  <section class="settings-page">
    <header class="page-heading">
      <div>
        <p class="eyebrow">{{ $t("settings.eyebrow") }}</p>
        <FTypography :header="1" spacing="none">{{
          $t("settings.title")
        }}</FTypography
        ><FTypography paragraphy type="secondary">{{
          $t("settings.description")
        }}</FTypography>
      </div>
      <div class="heading-mark" aria-hidden="true">
        <FIcon name="Settings" :size="22" />
      </div>
    </header>
    <p v-if="!isTauri()" class="runtime-note" role="status">
      <FIcon name="Info" :size="16" aria-label="" />{{
        $t("runtime.browserNotice")
      }}
    </p>
    <article class="settings-card">
      <div class="section-heading">
        <FIcon name="FolderOpen" :size="18" aria-label="" />
        <div>
          <FTypography :header="3" spacing="none">{{
            $t("settings.gameDirectoryTitle")
          }}</FTypography
          ><FTypography paragraphy type="secondary">{{
            $t("settings.gameDirectoryDescription")
          }}</FTypography>
        </div>
      </div>
      <form class="settings-form" @submit.prevent="saveGameDirectory">
        <label for="game-directory">{{
          $t("settings.gameDirectoryLabel")
        }}</label>
        <div class="form-row">
          <input
            id="game-directory"
            v-model="gamePath"
            autocomplete="off"
            :placeholder="$t('settings.gameDirectoryPlaceholder')"
          /><button class="primary-button" type="submit" :disabled="loading">
            <FIcon name="Save" :size="16" aria-label="" />{{
              loading ? $t("common.loading") : $t("settings.save")
            }}
          </button>
        </div>
        <p v-if="saved" class="success-message" role="status">
          {{ $t("settings.saved") }}
        </p>
        <p
          v-if="status?.gameDirectory && !status.gameDirectory.accessible"
          class="warning-message"
          role="alert"
        >
          {{ $t("settings.gameDirectoryUnavailable") }}
        </p>
      </form>
      <div v-if="detection" class="candidate-list">
        <FTypography :header="4" spacing="none">{{
          $t("settings.detectedCandidates")
        }}</FTypography>
        <div
          v-for="candidate in detection.candidates"
          :key="candidate.path"
          class="candidate"
        >
          <FIcon
            :name="candidate.hasPackageMarker ? 'CircleCheck' : 'CircleAlert'"
            :color="
              candidate.hasPackageMarker
                ? 'var(--success)'
                : 'var(--muted-foreground)'
            "
            :size="16"
            aria-label=""
          /><span>{{ candidate.path }}</span
          ><small>{{
            candidate.hasPackageMarker
              ? $t("settings.packageMarkerFound")
              : $t("settings.noPackageMarker")
          }}</small>
        </div>
      </div>
    </article>
    <article class="settings-card">
      <div class="section-heading">
        <FIcon name="BookOpen" :size="18" aria-label="" />
        <div>
          <FTypography :header="3" spacing="none">{{
            $t("settings.workspaceTitle")
          }}</FTypography
          ><FTypography paragraphy type="secondary">{{
            $t("settings.workspaceDescription")
          }}</FTypography>
        </div>
      </div>
      <div class="workspace-status">
        <span>{{
          workspace?.rootPath ?? $t("settings.workspaceNotConfigured")
        }}</span
        ><RouterLink class="secondary-button" to="/workspace"
          ><FIcon name="FolderOpen" :size="16" aria-label="" />{{
            $t("settings.openWorkspace")
          }}</RouterLink
        >
      </div>
    </article>
    <p v-if="error" class="error-message" role="alert">{{ error }}</p>
  </section>
</template>

<style scoped>
.settings-page {
  display: grid;
  gap: 24px;
  max-width: 820px;
}
.page-heading {
  align-items: flex-start;
  display: flex;
  justify-content: space-between;
}
.eyebrow {
  color: var(--primary);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.08em;
  margin: 0 0 10px;
  text-transform: uppercase;
}
.heading-mark {
  align-items: center;
  background: var(--accent);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  color: var(--primary);
  display: flex;
  height: 52px;
  justify-content: center;
  width: 52px;
}
.runtime-note,
.settings-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
}
.runtime-note {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 12px;
  gap: 8px;
  padding: 12px 14px;
}
.settings-card {
  display: grid;
  gap: 20px;
  padding: 22px;
}
.section-heading {
  align-items: flex-start;
  display: flex;
  gap: 12px;
}
.section-heading > svg {
  color: var(--primary);
  flex: none;
  margin-top: 3px;
}
.settings-form {
  border-top: 1px solid var(--border);
  padding-top: 18px;
}
.settings-form label {
  color: var(--muted-foreground);
  display: block;
  font-size: 12px;
  margin-bottom: 7px;
}
.form-row {
  display: flex;
  gap: 8px;
}
.form-row input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  min-width: 0;
  padding: 10px 11px;
  width: 100%;
}
.form-row input:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.primary-button,
.secondary-button {
  align-items: center;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: inline-flex;
  font-size: 12px;
  font-weight: 700;
  gap: 7px;
  justify-content: center;
  min-height: 36px;
  padding: 0 12px;
  text-decoration: none;
  white-space: nowrap;
}
.primary-button {
  background: var(--primary);
  border: 1px solid var(--primary);
  color: var(--primary-foreground);
}
.secondary-button {
  background: var(--surface-hover);
  border: 1px solid var(--border);
  color: var(--foreground);
}
.primary-button:disabled {
  cursor: wait;
  opacity: 0.6;
}
.success-message {
  color: var(--success);
  font-size: 12px;
  margin: 9px 0 0;
}
.warning-message,
.error-message {
  color: var(--danger);
  font-size: 12px;
  margin: 9px 0 0;
}
.candidate-list {
  display: grid;
  gap: 10px;
}
.candidate {
  align-items: center;
  background: var(--surface-elevated);
  border-radius: var(--radius-sm);
  display: grid;
  gap: 8px;
  grid-template-columns: auto 1fr auto;
  padding: 10px 12px;
}
.candidate span {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.candidate small {
  color: var(--muted-foreground);
  font-size: 11px;
}
.workspace-status {
  align-items: center;
  background: var(--surface-elevated);
  border-radius: var(--radius-sm);
  display: flex;
  font-size: 12px;
  gap: 14px;
  justify-content: space-between;
  padding: 12px;
}
.workspace-status > span {
  color: var(--muted-foreground);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
@media (max-width: 600px) {
  .form-row,
  .workspace-status {
    align-items: stretch;
    flex-direction: column;
  }
  .candidate {
    grid-template-columns: auto 1fr;
  }
  .candidate small {
    grid-column: 2;
  }
  .form-row button {
    width: 100%;
  }
}
</style>
