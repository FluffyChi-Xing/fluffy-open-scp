<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import { isTauri, tauriApi } from "@/api";
import type { GameDirectoryDetection } from "@/api/tauri";

/**
 * 首次安装引导（全屏独立页）：确认游戏安装目录。
 * 文档工作区与模组根自 2026-10-29 起由启动自动落盘（internal_dirs），
 * 引导只保留必须由用户指定的游戏目录。绿色免安装版首次启动自动路由到
 * 本页；完成写入 onboarding 标志。
 */
const router = useRouter();
const { t } = useI18n();

const busy = ref(false);
const error = ref("");
const detection = ref<GameDirectoryDetection | null>(null);
const gamePath = ref("");
const gameOk = ref(false);

onMounted(async () => {
  if (!isTauri()) return;
  try {
    const settings = await tauriApi.settings.get();
    gamePath.value = settings.gameDataPath ?? "";
    gameOk.value = Boolean(settings.gameDataPath);
    void loadCandidates();
  } catch {
    // 首次启动读取失败按未配置处理
  }
});

function errorMessage(cause: unknown): string {
  return cause && typeof cause === "object" && "message" in cause
    ? String(cause.message)
    : String(cause);
}

async function chooseGame() {
  const path = await tauriApi.workspace.pickDirectory(
    t("onboarding.pickTitle"),
  );
  if (!path) return;
  busy.value = true;
  error.value = "";
  try {
    await tauriApi.settings.setGameDirectory(path);
    gamePath.value = path;
    gameOk.value = true;
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function useCandidate(path: string) {
  busy.value = true;
  error.value = "";
  try {
    await tauriApi.settings.setGameDirectory(path);
    gamePath.value = path;
    gameOk.value = true;
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function loadCandidates() {
  if (detection.value || !isTauri()) return;
  try {
    detection.value = await tauriApi.settings.detectGameDirectory();
  } catch {
    detection.value = null;
  }
}

async function finish() {
  try {
    await tauriApi.studio.completeOnboarding();
  } catch (cause) {
    error.value = errorMessage(cause);
  }
  await router.replace("/");
}
</script>

<template>
  <section class="onboarding">
    <div class="wizard-card">
      <header class="wizard-head">
        <p class="eyebrow">{{ $t("onboarding.eyebrow") }}</p>
        <FTypography :header="1" spacing="none">{{
          $t("onboarding.title")
        }}</FTypography>
        <p class="lead">{{ $t("onboarding.description") }}</p>
      </header>

      <article class="step-card">
        <h2>{{ $t("onboarding.stepGameTitle") }}</h2>
        <p class="step-desc">{{ $t("onboarding.stepGameDesc") }}</p>

        <div class="path-row">
          <code class="path" :class="{ ok: gameOk }">{{
            gamePath || $t("onboarding.notConfigured")
          }}</code>
          <span v-if="gameOk" class="ok-flag"
            ><FIcon name="CircleCheck" :size="14" aria-label="" />{{
              $t("onboarding.configured")
            }}</span
          >
          <button
            class="pick"
            type="button"
            :disabled="busy"
            @click="chooseGame"
          >
            <FIcon name="FolderOpen" :size="13" aria-label="" />
            {{ $t("onboarding.pick") }}
          </button>
        </div>
        <div v-if="detection?.candidates.length" class="candidates">
          <p class="candidates-label">{{ $t("onboarding.gameCandidates") }}</p>
          <button
            v-for="candidate in detection.candidates"
            :key="candidate.path"
            class="candidate"
            type="button"
            :disabled="!candidate.isDirectory"
            @click="useCandidate(candidate.path)"
          >
            <FIcon
              :name="candidate.hasPackageMarker ? 'CircleCheck' : 'Folder'"
              :size="14"
              aria-label=""
            />
            <code>{{ candidate.path }}</code>
            <small v-if="!candidate.isDirectory">{{
              $t("onboarding.candidateInvalid")
            }}</small>
          </button>
        </div>

        <p class="skip-hint">{{ $t("onboarding.skipHint") }}</p>
        <p v-if="error" class="error" role="alert">{{ error }}</p>
      </article>

      <footer class="wizard-foot">
        <span class="spacer"></span>
        <button class="primary" type="button" @click="finish">
          <FIcon name="Check" :size="13" aria-label="" />
          {{ $t("onboarding.finish") }}
        </button>
      </footer>
    </div>
  </section>
</template>

<style scoped>
.onboarding {
  align-items: center;
  background:
    radial-gradient(
      800px 400px at 70% -10%,
      color-mix(in srgb, var(--primary) 18%, transparent),
      transparent
    ),
    var(--surface);
  display: flex;
  justify-content: center;
  min-height: 100vh;
  padding: 24px;
}
.wizard-card {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
  display: grid;
  gap: 20px;
  max-width: 620px;
  padding: 32px;
  width: 100%;
}
.eyebrow {
  color: var(--primary);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.08em;
  margin: 0 0 8px;
}
.lead {
  color: var(--muted-foreground);
  font-size: 13px;
  margin: 8px 0 0;
}
.step-rail {
  align-items: center;
  display: flex;
  gap: 8px;
}
.dot {
  background: var(--border);
  border-radius: 999px;
  height: 8px;
  transition:
    background 160ms ease,
    width 160ms ease;
  width: 8px;
}
.dot.active {
  background: var(--primary);
  width: 22px;
}
.dot.done {
  background: color-mix(in srgb, var(--primary) 55%, var(--border));
}
.step-count {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin-inline-start: auto;
}
.step-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: grid;
  gap: 10px;
  padding: 20px;
}
.step-card h2 {
  font-size: 15px;
  margin: 0;
}
.step-desc {
  color: var(--muted-foreground);
  font-size: 12.5px;
  margin: 0;
}
.path-row {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}
.path {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  flex: 1;
  font-size: 12px;
  min-height: 32px;
  overflow: hidden;
  padding: 6px 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.path.ok {
  border-color: color-mix(in srgb, var(--success) 50%, var(--border));
}
.ok-flag {
  align-items: center;
  color: var(--success);
  display: inline-flex;
  font-size: 11.5px;
  gap: 4px;
}
.pick {
  align-items: center;
  background: var(--primary);
  border: 1px solid var(--primary);
  border-radius: var(--radius-sm);
  color: #fff;
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 6px;
  min-height: 32px;
  padding: 0 14px;
}
.candidates {
  display: grid;
  gap: 6px;
}
.candidates-label {
  color: var(--subtle-foreground);
  font-size: 10.5px;
  letter-spacing: 0.08em;
  margin: 4px 0 0;
  text-transform: uppercase;
}
.candidate {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 8px;
  min-height: 32px;
  padding: 4px 10px;
  text-align: start;
}
.candidate:hover:not(:disabled) {
  background: var(--surface-hover);
}
.candidate:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.candidate code {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.candidate small {
  color: var(--warning);
}
.skip-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 0;
}
.error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  overflow-wrap: anywhere;
}
.wizard-foot {
  align-items: center;
  display: flex;
  gap: 10px;
}
.spacer {
  flex: 1;
}
.ghost,
.primary {
  align-items: center;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 6px;
  min-height: 32px;
  padding: 0 16px;
}
.ghost {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--muted-foreground);
}
.ghost:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.primary {
  background: var(--primary);
  border: 1px solid var(--primary);
  color: #fff;
}
</style>
