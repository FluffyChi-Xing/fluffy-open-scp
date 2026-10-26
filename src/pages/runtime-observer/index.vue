<script setup lang="ts">
import {
  computed,
  onActivated,
  onDeactivated,
  onBeforeUnmount,
  onMounted,
  ref,
} from "vue";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "reka-ui";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FButton from "@/components/ui/FButton.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FSelect from "@/components/ui/FSelect.vue";
import Switch from "@/components/ui/switch/Switch.vue";
import LogStream from "./LogStream.vue";
import DiagnosticWorkbench from "./DiagnosticWorkbench.vue";
import {
  runtimeObserver,
  type GameProcess,
  type ObserverStatus,
} from "@/api/runtimeObserver";
const { t, te } = useI18n();
const mode = ref("live");
const empty = (): ObserverStatus => ({
  running: false,
  records: 0,
  synced: 0,
  recent: [],
  eventCounts: {},
});
const status = ref(empty());
const captureEpoch = ref(0);
const processes = ref<GameProcess[]>([]);
const selectedPid = ref<number>();
const profile = ref("balanced");
const captureScope = ref<"ui" | "building" | "agent">("ui");
const captureTarget = ref("all");
const frameUrl = ref("");
const captureFrames = ref(true);
const captureScopes = computed(() => [
  { value: "ui", label: t("runtimeObserver.scope.ui.label"), hint: t("runtimeObserver.scope.ui.hint") },
  { value: "building", label: t("runtimeObserver.scope.building.label"), hint: t("runtimeObserver.scope.building.hint") },
  { value: "agent", label: t("runtimeObserver.scope.agent.label"), hint: t("runtimeObserver.scope.agent.hint") },
] as const);
const captureTargets = computed(() => captureScope.value === "building"
  ? [{ value: "school", label: t("runtimeObserver.target.school") }, { value: "omegaco", label: t("runtimeObserver.target.omegaco") }]
  : captureScope.value === "agent"
    ? [{ value: "police", label: t("runtimeObserver.target.police") }]
    : [{ value: "all", label: t("runtimeObserver.target.all") }]);
const observation = computed(() => `${captureScope.value}:${captureTarget.value}`);
const error = ref("");
const displayError = computed(() => {
  try {
    const detail = JSON.parse(error.value);
    const key = `runtimeObserver.errors.${detail.code}`;
    return te(key) ? t(key) : detail.message || error.value;
  } catch {
    return error.value;
  }
});
const busy = ref(false);
const stopping = ref(false);
const processMenu = ref(false);
const processMenuOpen = computed({
  get: () => processMenu.value && !busy.value && !status.value.running,
  set: (value: boolean) => {
    processMenu.value = value && !busy.value && !status.value.running;
  },
});
const selectedProcess = computed(() =>
  processes.value.find((p) => p.pid === selectedPid.value),
);
const profileOptions = computed(() =>
  ["balanced", "ui-events", "bridge-commands", "engine-messages"].map(
    (value) => ({
      value,
      label: t(`runtimeObserver.profile.${value}.label`),
    }),
  ),
);
let timer: ReturnType<typeof setTimeout> | undefined;
let active = false;
let refreshing = false;
let starting = false;
let frozen = false;
let pollVersion = 0;
const phase = computed(() =>
  stopping.value && status.value.running
    ? "stopping"
    : status.value.running
      ? status.value.status?.ready
        ? "running"
        : "preparing"
      : "idle",
);
const events = computed(() =>
  Object.values(status.value.eventCounts).reduce((a, b) => a + b, 0),
);
const latestDataResult = (requestName: string) => [...status.value.recent].reverse().find((row) => row.requestName === requestName && row.channel === "result");
const gamePaused = computed<boolean | undefined>(() => {
  if (typeof status.value.gamePaused === "boolean") return status.value.gamePaused;
  const row = [...status.value.recent].reverse().find((candidate) => typeof candidate.requestName === "string" && candidate.requestName.startsWith("globalFlags") && candidate.channel === "result");
  const value = row?.result && typeof row.result === "object" && "value" in row.result ? row.result.value : undefined;
  return typeof value === "boolean" ? value : undefined;
});
const gameTime = computed(() => {
  const source = typeof status.value.gameClockSeconds === "number"
    ? status.value.gameClockSeconds
    : status.value.gameTimeSeconds;
  if (typeof source === "number") {
    const total = Math.max(0, Math.floor(source));
    return `${String(Math.floor(total / 3600)).padStart(2, "0")}:${String(Math.floor(total / 60) % 60).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
  }
  const row = latestDataResult("totalElapsedGameTime");
  const value = row?.result && typeof row.result === "object" && "value" in row.result ? Number(row.result.value) : NaN;
  if (!Number.isFinite(value)) return undefined;
  const total = Math.max(0, Math.floor(value));
  return `${String(Math.floor(total / 3600)).padStart(2, "0")}:${String(Math.floor(total / 60) % 60).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
});
const gameTimeIsClock = computed(() => typeof status.value.gameClockSeconds === "number");
function message(e: unknown) {
  return e && typeof e === "object" && "message" in e
    ? String(e.message)
    : String(e);
}
async function scan() {
  busy.value = true;
  try {
    processes.value = await runtimeObserver.listProcesses();
    if (!processes.value.some((p) => p.pid === selectedPid.value))
      selectedPid.value = processes.value[0]?.pid;
  } catch (e) {
    error.value = message(e);
  } finally {
    busy.value = false;
  }
}
async function refresh() {
  if (!active || refreshing) return;
  refreshing = true;
  const epoch = captureEpoch.value;
  const version = pollVersion;
  try {
    if (starting || stopping.value || frozen) return;
    const next = await runtimeObserver.status();
    if (!active || epoch !== captureEpoch.value || version !== pollVersion)
      return;
    status.value = next;
    if (captureFrames.value && next.pid) {
      try {
        const frame = await runtimeObserver.frame(next.pid);
        frameUrl.value = `data:${frame.mime};base64,${frame.dataBase64}`;
      } catch {
        // A minimized or transitioning window can fail one frame; keep the last good frame.
      }
    }
    if (!next.running) stopping.value = false;
    if (next.status?.error) error.value = String(next.status.error);
    if (next.directory) await runtimeObserver.sync();
  } catch (e) {
    if (active && epoch === captureEpoch.value && version === pollVersion)
      error.value = message(e);
  } finally {
    refreshing = false;
    if (active) timer = setTimeout(refresh, 1000);
  }
}
async function start() {
  if (
    !selectedPid.value ||
    busy.value ||
    status.value.running ||
    stopping.value
  )
    return;
  busy.value = true;
  error.value = "";
  stopping.value = false;
  starting = true;
  frozen = false;
  pollVersion += 1;
  captureEpoch.value += 1;
  status.value = empty();
  frameUrl.value = "";
  processMenu.value = false;
  try {
    const next = await runtimeObserver.start(selectedPid.value, 60, profile.value, observation.value);
    if (active) status.value = next;
    else await runtimeObserver.stop();
  } catch (e) {
    error.value = message(e);
  } finally {
    starting = false;
    busy.value = false;
  }
}
async function stop() {
  if (stopping.value) return;
  stopping.value = true;
  pollVersion += 1;
  const epoch = captureEpoch.value;
  error.value = "";
  try {
    const next = await runtimeObserver.stop();
    if (!active || epoch !== captureEpoch.value) return;
    // The backend removes its temporary trace after returning this snapshot.
    // Keep the page's final snapshot for reading instead of polling the deleted trace.
    status.value = next.recent.length
      ? next
      : {
          ...next,
          recent: status.value.recent,
          records: status.value.records,
          eventCounts: status.value.eventCounts,
        };
    frozen = !next.running;
  } catch (e) {
    if (active && epoch === captureEpoch.value) error.value = message(e);
  } finally {
    if (active && epoch === captureEpoch.value) stopping.value = false;
  }
}
function activate() {
  if (active) return;
  active = true;
  void scan();
  void refresh();
}
function release() {
  if (!active) return;
  active = false;
  frozen = false;
  pollVersion += 1;
  captureEpoch.value += 1;
  error.value = "";
  processMenu.value = false;
  clearTimeout(timer);
  void runtimeObserver.stop().catch(() => {});
  status.value = empty();
  frameUrl.value = "";
}
onMounted(activate);
onActivated(activate);
onDeactivated(release);
onBeforeUnmount(release);
</script>
<template>
  <TabsRoot v-model="mode" as="main" class="console-page">
    <header class="console-header">
      <h1>{{ t("runtimeObserver.title") }}</h1>
      <span v-if="mode === 'live'">{{ t("runtimeObserver.memoryOnly") }}</span>
      <TabsList class="mode-tabs" :aria-label="t('runtimeObserver.title')">
        <TabsTrigger value="live">{{ t("diagnostics.liveTab") }}</TabsTrigger>
        <TabsTrigger value="static">{{
          t("diagnostics.staticTab")
        }}</TabsTrigger>
      </TabsList>
    </header>
    <div v-show="mode === 'live'">
      <TabsContent value="live" force-mount class="mode-panel">
        <p v-if="error" class="error" role="alert">{{ displayError }}</p>

        <section class="summary">
          <div class="preview" :class="{ ready: frameUrl }">
            <img v-if="frameUrl" :src="frameUrl" :alt="t('runtimeObserver.frame')" />
            <span v-else>{{ t("runtimeObserver.previewUnavailable") }}</span>
          </div>
          <div class="session">
            <div class="runtime-strip">
              <span class="runtime-strip-title">{{ t("runtimeObserver.runtimeStatus") }}</span>
              <span class="runtime-pill" :class="{ paused: gamePaused === true, active: gamePaused === false }">
                <span class="runtime-dot" aria-hidden="true"></span>
                {{ gamePaused === true ? t("runtimeObserver.paused") : gamePaused === false ? t("runtimeObserver.playing") : t("runtimeObserver.unknownStatus") }}
              </span>
              <span class="runtime-time"><span>{{ t(gameTimeIsClock ? "runtimeObserver.gameClock" : "runtimeObserver.gameTime") }}</span><strong>{{ gameTime ?? "—" }}</strong></span>
              <label class="frame-toggle"><Switch v-model="captureFrames" :aria-label="t('runtimeObserver.captureFrame')" /><span>{{ t("runtimeObserver.captureFrame") }}</span></label>
              <div class="header-process">
                <FDropdown v-model:open="processMenuOpen" :width="240">
                  <template #trigger>
                    <FButton id="observer-process" class="process-trigger" variant="secondary" :disabled="status.running || busy || !processes.length" :aria-label="t('runtimeObserver.processes')" aria-haspopup="menu" :aria-expanded="processMenuOpen">
                      {{ selectedProcess ? `${selectedProcess.name} · PID ${selectedProcess.pid}` : t("runtimeObserver.noProcesses") }}
                      <FIcon class="process-chevron" name="ChevronDown" :size="14" aria-label="" />
                    </FButton>
                  </template>
                  <button v-for="p in processes" :key="p.pid" type="button" role="menuitemradio" :aria-checked="p.pid === selectedPid" @click="selectedPid = p.pid; processMenuOpen = false">
                    {{ p.name }} · PID {{ p.pid }}
                  </button>
                </FDropdown>
                <FButton variant="secondary" :loading="busy" :disabled="status.running" @click="scan">{{ t("runtimeObserver.scan") }}</FButton>
              </div>
            </div>
            <h2 class="capture-title">{{ t("diagnostics.liveTitle") }}</h2>
            <p class="capture-hint">{{ t("diagnostics.liveHint") }}</p>
          </div>
          <dl class="metrics">
            <div class="metric-card">
              <dt>{{ t("runtimeObserver.records") }}</dt>
              <dd>{{ status.records }}</dd>
            </div>
            <div class="metric-card">
              <dt>{{ t("runtimeObserver.eventTotal") }}</dt>
              <dd>{{ events }}</dd>
            </div>
            <div class="metric-card">
              <dt>{{ t("runtimeObserver.eventCamera") }}</dt>
              <dd>
                {{
                  (status.eventCounts["205419630"] ?? 0) +
                  (status.eventCounts["205419631"] ?? 0)
                }}
              </dd>
            </div>
            <div class="metric-card">
              <dt>{{ t("runtimeObserver.synced") }}</dt>
              <dd>{{ status.synced }}</dd>
            </div>
          </dl>
        </section>

        <section class="capture-toolbar" :aria-label="t('runtimeObserver.control')">
          <div class="capture-filter" :aria-label="t('runtimeObserver.captureScope')">
            <div class="scope-tabs" role="tablist">
              <button v-for="scope in captureScopes" :key="scope.value" type="button" role="tab" :aria-selected="captureScope === scope.value" :class="{ active: captureScope === scope.value }" @click="captureScope = scope.value; captureTarget = 'all'">{{ scope.label }}</button>
            </div>
            <p class="scope-hint">{{ captureScopes.find((scope) => scope.value === captureScope)?.hint }}</p>
            <div class="target-radios" role="radiogroup" :aria-label="t('runtimeObserver.captureTarget')">
              <label v-for="target in captureTargets" :key="target.value" class="target-radio"><input v-model="captureTarget" type="radio" name="capture-target" :value="target.value" /><span>{{ target.label }}</span></label>
            </div>
          </div>
          <div class="toolbar-profile">
            <label class="profile-picker"><span>{{ t("runtimeObserver.profileLabel") }}</span><FSelect v-model="profile" :options="profileOptions" :disabled="status.running || busy" :aria-label="t('runtimeObserver.profileLabel')" /></label>
            <p class="profile-description">{{ t(`runtimeObserver.profile.${profile}.description`) }}</p>
          </div>
          <div class="toolbar-actions">
            <FButton :disabled="status.running || !selectedPid" :loading="busy" @click="start">{{ t("runtimeObserver.start") }}</FButton>
            <FButton variant="secondary" :disabled="!status.running || stopping" @click="stop">{{ t("runtimeObserver.stop") }}</FButton>
            <span class="phase" role="status">{{ t("runtimeObserver." + phase) }}</span>
          </div>
        </section>

        <LogStream :key="captureEpoch" :rows="status.recent" :scope="captureScope" :target="captureTarget" />
      </TabsContent>
    </div>
    <div v-show="mode === 'static'">
      <TabsContent value="static" force-mount class="mode-panel">
        <DiagnosticWorkbench />
      </TabsContent>
    </div>
  </TabsRoot>
</template>
<style scoped>
.console-page {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
  color: var(--foreground);
}
.console-header {
  display: flex;
  align-items: center;
  gap: 16px;
  min-height: 28px;
  flex-wrap: wrap;
}
.mode-panel {
  display: grid;
  gap: 12px;
  min-width: 0;
}
.mode-tabs {
  display: inline-flex;
  margin-inline-start: auto;
}
.mode-tabs button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  min-height: 24px;
  padding: 2px 10px;
}
.mode-tabs button:first-child {
  border-end-end-radius: 0;
  border-start-end-radius: 0;
}
.mode-tabs button:last-child {
  border-end-start-radius: 0;
  border-start-start-radius: 0;
  margin-inline-start: -1px;
}
.mode-tabs button[data-state="active"] {
  color: var(--foreground);
  opacity: 0.85;
}
.console-header h1 {
  font-size: 16px;
  font-weight: 600;
  margin: 0;
}
.console-header span,
label,
dt {
  font-size: 12px;
  color: var(--muted-foreground);
}
.summary {
  display: grid;
  grid-template-columns: 320px minmax(360px, 1fr) 240px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--surface);
  align-items: stretch;
}
.preview {
  display: grid;
  place-items: center;
  background: var(--background);
  font-size: 12px;
  color: var(--muted-foreground);
  padding: 12px;
  border-right: 1px solid var(--border);
  box-sizing: border-box;
  aspect-ratio: 16 / 9;
  width: 100%;
  align-self: center;
  text-align: center;
}
.preview.ready { padding: 0; overflow: hidden; background: #050b16; }
.preview img { display: block; width: 100%; height: 100%; object-fit: contain; outline: 1px solid oklch(1 0 0 / 0.1); }
.capture-title {
  font-size: 14px;
  margin: 0;
}
.capture-hint {
  font-size: 12px;
  color: var(--muted-foreground);
  line-height: 1.6;
  margin: 0;
}
.session {
  padding: 12px;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  gap: 6px;
}
.runtime-strip {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  padding-bottom: 8px;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}
.runtime-strip-title { color: var(--muted-foreground); font-size: 11px; }
.runtime-pill, .runtime-time { display: inline-flex; align-items: center; gap: 6px; min-height: 24px; padding: 0 8px; border: 1px solid var(--border); border-radius: 999px; background: var(--background); font-size: 11px; }
.runtime-pill.active { color: var(--success, #72c7a0); border-color: color-mix(in srgb, var(--success, #72c7a0) 45%, var(--border)); }
.runtime-pill.paused { color: var(--warning, #d6a85e); border-color: color-mix(in srgb, var(--warning, #d6a85e) 45%, var(--border)); }
.runtime-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--muted-foreground); }
.runtime-pill.active .runtime-dot { background: var(--success, #72c7a0); }
.runtime-pill.paused .runtime-dot { background: var(--warning, #d6a85e); }
.runtime-time span { color: var(--muted-foreground); }
.runtime-time strong { font: 600 12px ui-monospace, Consolas, monospace; color: var(--foreground); }
.header-process { display: inline-flex; align-items: center; gap: 6px; margin-left: auto; }
.header-process .process-trigger { min-height: 28px; font-size: 11px; }
.header-process :deep(button) { white-space: nowrap; }
.frame-toggle { display: inline-flex; align-items: center; gap: 6px; color: var(--muted-foreground); font-size: 11px; cursor: pointer; }
.controls {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}
.profile-picker {
  display: flex;
  align-items: center;
  gap: 8px;
  width: min(100%, 260px);
}
.profile-picker :deep(.f-select-anchor) {
  flex: 0 1 150px;
  width: 150px;
}
.profile-picker :deep(.f-select) {
  min-height: 30px;
  height: 30px;
  padding: 0 8px;
  font-size: 12px;
  line-height: 1.2;
}
.profile-description {
  max-width: 420px;
  margin: 0;
  color: var(--muted-foreground);
  font-size: 12px;
  line-height: 1.4;
}
.process-chevron {
  display: block;
  flex: none;
  align-self: center;
}
.process-trigger {
  min-width: 180px;
  justify-content: space-between;
  gap: 12px;
}
.controls :deep(.f-button) {
  min-height: 30px;
  height: 30px;
  padding: 0 10px;
  font-size: 12px;
  font-weight: 500;
}
.phase {
  font-size: 12px;
  color: var(--muted-foreground);
}
.capture-filter { display: grid; gap: 6px; margin-top: 2px; }
.scope-tabs { display: flex; gap: 4px; flex-wrap: wrap; }
.scope-tabs button { min-height: 28px; padding: 0 10px; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--background); color: var(--muted-foreground); cursor: pointer; font: 12px var(--font-sans); transition: color 120ms ease, background-color 120ms ease, border-color 120ms ease; }
.scope-tabs button.active { color: var(--foreground); border-color: color-mix(in srgb, var(--accent) 65%, var(--border)); background: color-mix(in srgb, var(--accent) 14%, var(--background)); }
.scope-hint { margin: 0; color: var(--muted-foreground); font-size: 11px; }
.capture-notice { margin: 0; color: var(--warning, #d6a85e); font-size: 11px; line-height: 1.35; max-width: 360px; }
.target-radios { display: flex; gap: 12px; flex-wrap: wrap; }
.target-radio { display: inline-flex; align-items: center; gap: 6px; min-height: 24px; color: var(--foreground); cursor: pointer; }
.target-radio input {
  appearance: none;
  width: 14px;
  height: 14px;
  margin: 0;
  flex: none;
  border: 1px solid var(--muted-foreground);
  border-radius: 999px;
  background: var(--background);
  box-shadow: inset 0 0 0 3px var(--background);
  transition: border-color 120ms ease, background-color 120ms ease, box-shadow 120ms ease;
}
.target-radio input:checked {
  border-color: var(--accent);
  background: var(--accent);
  box-shadow: inset 0 0 0 3px var(--background);
}
.target-radio input:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; }
.target-radio:hover input { border-color: var(--foreground); }
.metrics {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  padding: 12px;
  align-content: center;
  margin: 0;
  border-left: 1px solid var(--border);
}
.metric-card {
  min-width: 0;
  padding: 9px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--surface-elevated) 70%, var(--surface));
}
.metric-card dd { font-size: 18px; }
.capture-toolbar {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: 0 4px 14px rgb(0 0 0 / 10%);
}
.toolbar-process, .toolbar-profile, .toolbar-actions { display: grid; gap: 5px; }
.toolbar-process { min-width: 240px; }
.toolbar-profile { min-width: 260px; flex: 1 1 280px; }
.toolbar-actions { margin-inline-start: auto; display: flex; align-items: center; gap: 8px; }
.capture-toolbar .capture-filter { min-width: 230px; }
dd {
  font-size: 18px;
  font-variant-numeric: tabular-nums;
  margin: 4px 0 0;
}
.error {
  margin: 0;
  padding: 12px 16px;
  color: var(--danger);
  background: var(--surface);
  border: 1px solid var(--border);
  border-left: 3px solid var(--danger);
  border-radius: var(--radius-md);
  font: 12px/1.5 var(--font-sans);
  width: 100%;
  box-sizing: border-box;
  white-space: normal;
  overflow-wrap: anywhere;
  max-height: 120px;
  overflow: auto;
}
:where(button):focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
@media (max-width: 1000px) {
  .summary {
    grid-template-columns: 280px minmax(0, 1fr);
  }
  .metrics {
    grid-column: 1/-1;
    grid-template-columns: repeat(4, 1fr);
    border-left: 0;
    border-top: 1px solid var(--border);
  }
  .toolbar-actions { margin-inline-start: 0; }
}
@media (max-width: 600px) {
  .summary {
    grid-template-columns: 1fr;
  }
  .preview {
    width: min(100%, 240px);
    justify-self: center;
  }
  .metrics {
    grid-template-columns: 1fr 1fr;
  }
}
</style>
