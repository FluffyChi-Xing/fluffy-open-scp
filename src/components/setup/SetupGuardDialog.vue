<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import { isTauri, tauriApi } from "@/api";
import type { SetupPathStatus, SetupStatusResponse } from "@/api/tauri";

/**
 * 绿色免安装版的配置缺失守卫：
 * - 引导从未完成 → 跳转 /onboarding 全屏引导；
 * - 引导已完成但个别目录缺失/失效 → 弹居中 dialog，缺哪个配哪个。
 * 浏览器 demo 模式（非 Tauri）不检测。
 */
const router = useRouter();
const { t } = useI18n();
const open = ref(false);
const busy = ref(false);
const error = ref("");
const status = ref<SetupStatusResponse | null>(null);
/** 正在补配的路径键：workspace | game | modRoot。 */
const fixing = ref<null | "workspace" | "game" | "modRoot">(null);

const entries = computed(() => {
  const current = status.value;
  if (!current)
    return [] as { key: string; label: string; value: SetupPathStatus }[];
  return [
    {
      key: "workspace",
      label: t("setupDialog.workspace"),
      value: current.workspace,
    },
    { key: "game", label: t("setupDialog.game"), value: current.game },
    { key: "modRoot", label: t("setupDialog.modRoot"), value: current.modRoot },
  ].filter((entry) => !(entry.value.path && entry.value.isDirectory));
});

const allConfigured = computed(
  () =>
    Boolean(status.value) &&
    [status.value?.workspace, status.value?.game, status.value?.modRoot].every(
      (entry) => Boolean(entry?.path) && Boolean(entry?.isDirectory),
    ),
);

onMounted(check);

async function check() {
  if (!isTauri()) return;
  try {
    const result = await tauriApi.studio.setupStatus();
    if (!result.onboardingCompleted) {
      // 首次安装：进全屏引导
      await router.replace("/onboarding");
      return;
    }
    status.value = result;
    if (!allConfigured.value) open.value = true;
  } catch {
    // 后端不可用时静默，不打扰使用
  }
}

function errorMessage(cause: unknown): string {
  return cause && typeof cause === "object" && "message" in cause
    ? String(cause.message)
    : String(cause);
}

async function pickAndSave(key: "workspace" | "game" | "modRoot") {
  fixing.value = key;
  error.value = "";
  try {
    const path = await tauriApi.workspace.pickDirectory(
      t("setupDialog.pickTitle"),
    );
    if (!path) return;
    busy.value = true;
    if (key === "workspace") await tauriApi.workspace.setRoot(path);
    else if (key === "game") await tauriApi.settings.setGameDirectory(path);
    else await tauriApi.studio.setModRoot(path);
    status.value = await tauriApi.studio.setupStatus();
    if (allConfigured.value) open.value = false;
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
    fixing.value = null;
  }
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="overlay"
      role="dialog"
      aria-modal="true"
      :aria-label="t('setupDialog.title')"
    >
      <div class="dialog">
        <header class="head">
          <span class="icon"
            ><FIcon name="CircleAlert" :size="18" aria-label=""
          /></span>
          <div>
            <strong>{{ $t("setupDialog.title") }}</strong>
            <p>{{ $t("setupDialog.description") }}</p>
          </div>
        </header>
        <ul class="missing-list">
          <li v-for="entry in entries" :key="entry.key" class="missing-row">
            <span class="label">{{ entry.label }}</span>
            <code v-if="entry.value.path" class="path">{{
              entry.value.path
            }}</code>
            <span v-else class="path unset">{{ $t("setupDialog.unset") }}</span>
            <button
              class="fix"
              type="button"
              :disabled="busy"
              @click="
                pickAndSave(entry.key as 'workspace' | 'game' | 'modRoot')
              "
            >
              <FIcon name="FolderOpen" :size="13" aria-label="" />
              {{ $t("setupDialog.pick") }}
            </button>
          </li>
        </ul>
        <p v-if="error" class="error" role="alert">{{ error }}</p>
        <footer class="foot">
          <button class="later" type="button" @click="open = false">
            {{ $t("setupDialog.later") }}
          </button>
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.overlay {
  align-items: center;
  background: oklch(0.1 0.01 260 / 0.45);
  display: flex;
  inset: 0;
  justify-content: center;
  position: fixed;
  z-index: 90;
}
.dialog {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
  display: grid;
  gap: 14px;
  max-width: 560px;
  padding: 22px;
  width: min(560px, 92vw);
}
.head {
  display: flex;
  gap: 12px;
}
.icon {
  align-items: center;
  background: color-mix(in srgb, var(--warning) 15%, transparent);
  border-radius: 50%;
  color: var(--warning);
  display: inline-flex;
  flex: none;
  height: 38px;
  justify-content: center;
  width: 38px;
}
.head strong {
  font-size: 14px;
}
.head p {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 4px 0 0;
}
.missing-list {
  display: grid;
  gap: 8px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.missing-row {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: grid;
  gap: 10px;
  grid-template-columns: 96px minmax(0, 1fr) auto;
  padding: 8px 10px;
}
.label {
  color: var(--muted-foreground);
  font-size: 11.5px;
}
.path {
  font-size: 11.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.path.unset {
  color: var(--warning);
}
.fix {
  align-items: center;
  background: var(--primary);
  border: 1px solid var(--primary);
  border-radius: var(--radius-sm);
  color: #fff;
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11.5px;
  gap: 5px;
  min-height: 28px;
  padding: 0 10px;
}
.error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  overflow-wrap: anywhere;
}
.foot {
  display: flex;
  justify-content: flex-end;
}
.later {
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  min-height: 28px;
  padding: 0 12px;
}
</style>
