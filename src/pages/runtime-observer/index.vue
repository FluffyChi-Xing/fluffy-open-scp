<script setup lang="ts">
import {
  computed,
  onActivated,
  onDeactivated,
  onBeforeUnmount,
  onMounted,
  ref,
} from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FButton from "@/components/ui/FButton.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import LogStream from "./LogStream.vue";
import {
  runtimeObserver,
  type GameProcess,
  type ObserverStatus,
} from "@/api/runtimeObserver";
const { t, te } = useI18n();
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
    if (!active || epoch !== captureEpoch.value || version !== pollVersion) return;
    status.value = next;
    if (!next.running) stopping.value = false;
    if (next.status?.error) error.value = String(next.status.error);
    if (next.directory) await runtimeObserver.sync();
  } catch (e) {
    if (active && epoch === captureEpoch.value && version === pollVersion) error.value = message(e);
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
    const next = await runtimeObserver.start(selectedPid.value, 60);
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
    status.value = next.recent.length ? next : {
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
  <main class="console-page">
    <header class="console-header">
      <h1>{{ t("runtimeObserver.title") }}</h1>
      <span>{{ t("runtimeObserver.memoryOnly") }}</span>
    </header>
    <p v-if="error" class="error" role="alert">{{ displayError }}</p>
    <section class="summary">
      <div class="preview">
        <span>{{ t("runtimeObserver.previewUnavailable") }}</span>
      </div>
      <div class="session">
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
  </main>
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
  height: 28px;
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
  grid-template-columns: 200px minmax(300px, 1.3fr) minmax(240px, 1fr);
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
  min-height: 132px;
}
.session {
  padding: 14px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 10px;
}
.controls {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
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
  padding: 16px;
  margin: 0;
  border-left: 1px solid var(--border);
}
dd {
  font-size: 20px;
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
    grid-template-columns: 150px 1fr;
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
    display: none;
  }
  .metrics {
    grid-template-columns: 1fr 1fr;
  }
}
</style>
