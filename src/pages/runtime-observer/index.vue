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
  processMenu.value = false;
  try {
    const next = await runtimeObserver.start(selectedPid.value, 60, profile.value);
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
          <div class="preview">
            <span>{{ t("runtimeObserver.previewUnavailable") }}</span>
          </div>
          <div class="session">
            <h2 class="capture-title">{{ t("diagnostics.liveTitle") }}</h2>
            <p class="capture-hint">{{ t("diagnostics.liveHint") }}</p>
            <label for="observer-process">{{
              t("runtimeObserver.processes")
            }}</label>
            <div class="controls">
              <FDropdown v-model:open="processMenuOpen" :width="240">
                <template #trigger>
                  <FButton
                    id="observer-process"
                    class="process-trigger"
                    variant="secondary"
                    :disabled="status.running || busy || !processes.length"
                    :aria-label="t('runtimeObserver.processes')"
                    aria-haspopup="menu"
                    :aria-expanded="processMenuOpen"
                  >
                    {{
                      selectedProcess
                        ? `${selectedProcess.name} · PID ${selectedProcess.pid}`
                        : t("runtimeObserver.noProcesses")
                    }}
                    <FIcon
                      class="process-chevron"
                      name="ChevronDown"
                      :size="14"
                      aria-label=""
                    />
                  </FButton>
                </template>
                <button
                  v-for="p in processes"
                  :key="p.pid"
                  type="button"
                  role="menuitemradio"
                  :aria-checked="p.pid === selectedPid"
                  @click="
                    selectedPid = p.pid;
                    processMenuOpen = false;
                  "
                >
                  {{ p.name }} · PID {{ p.pid }}
                </button>
              </FDropdown>
              <FButton
                variant="secondary"
                :loading="busy"
                :disabled="status.running"
                @click="scan"
                >{{ t("runtimeObserver.scan") }}</FButton
              >
            </div>
            <div class="controls">
              <label class="profile-picker">
                <span>{{ t("runtimeObserver.profileLabel") }}</span>
                <FSelect
                  v-model="profile"
                  :options="profileOptions"
                  :disabled="status.running || busy"
                  :aria-label="t('runtimeObserver.profileLabel')"
                />
              </label>
              <p class="profile-description">
                {{ t(`runtimeObserver.profile.${profile}.description`) }}
              </p>
            </div>
            <div class="controls">
              <FButton
                :disabled="status.running || !selectedPid"
                :loading="busy"
                @click="start"
                >{{ t("runtimeObserver.start") }}</FButton
              >
              <FButton
                variant="secondary"
                :disabled="!status.running || stopping"
                @click="stop"
                >{{ t("runtimeObserver.stop") }}</FButton
              >
              <span class="phase" role="status">{{
                t("runtimeObserver." + phase)
              }}</span>
            </div>
          </div>
          <dl class="metrics">
            <div>
              <dt>{{ t("runtimeObserver.records") }}</dt>
              <dd>{{ status.records }}</dd>
            </div>
            <div>
              <dt>{{ t("runtimeObserver.eventTotal") }}</dt>
              <dd>{{ events }}</dd>
            </div>
            <div>
              <dt>{{ t("runtimeObserver.eventCamera") }}</dt>
              <dd>
                {{
                  (status.eventCounts["205419630"] ?? 0) +
                  (status.eventCounts["205419631"] ?? 0)
                }}
              </dd>
            </div>
            <div>
              <dt>{{ t("runtimeObserver.synced") }}</dt>
              <dd>{{ status.synced }}</dd>
            </div>
          </dl>
        </section>

        <LogStream :key="captureEpoch" :rows="status.recent" />
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
  grid-template-columns: 200px minmax(300px, 1fr) 240px;
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
  aspect-ratio: 4 / 3;
  width: 100%;
  align-self: center;
  text-align: center;
}
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
  justify-content: center;
  gap: 6px;
}
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
.metrics {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  padding: 12px;
  align-content: center;
  margin: 0;
  border-left: 1px solid var(--border);
}
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
    grid-template-columns: 200px minmax(0, 1fr);
  }
  .metrics {
    grid-column: 1/-1;
    grid-template-columns: repeat(4, 1fr);
    border-left: 0;
    border-top: 1px solid var(--border);
  }
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
