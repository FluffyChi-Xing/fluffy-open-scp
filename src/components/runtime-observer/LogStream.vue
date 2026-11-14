<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import Switch from "@/components/ui/switch/Switch.vue";
import FSelect from "@/components/ui/FSelect.vue";
import FButton from "@/components/ui/FButton.vue";
import { save } from "@tauri-apps/plugin-dialog";
import { command } from "@/api/tauri";
import { useToast } from "@/composables/useToast";
const props = defineProps<{
  rows: Record<string, unknown>[];
  metric?: string;
  scope?: "ui" | "building" | "agent";
  target?: string;
}>();
const { t } = useI18n();
const toast = useToast();
const query = ref("");
const instance = ref("");
const instanceId = useId();
const viewport = ref<HTMLElement>();
const follow = ref(true);
const history = ref<Record<string, unknown>[]>([]);
watch(
  () => props.rows,
  (rows) => {
    if (!rows.length) {
      history.value = [];
      return;
    }
    const merged = new Map(history.value.map((row) => [row.sequence, row]));
    for (const row of rows) merged.set(row.sequence, row);
    history.value = Array.from(merged.values())
      .sort((a, b) => Number(a.sequence) - Number(b.sequence))
      .slice(-1000);
  },
  { immediate: true },
);
function text(row: Record<string, unknown>) {
  if (row.channel === "asset.resource") {
    const format = (value: unknown) => {
      if (row.source === "graphics-interiors") {
        const state = value as {
          target: number;
          actual: number;
          intact: number;
        } | null;
        return state
          ? t("assetCapture.lightReading", state)
          : t("assetCapture.missingController");
      }
      const bin = value as { amount: number; capacity: number } | null;
      return bin
        ? `${bin.amount} / ${bin.capacity}`
        : t("assetCapture.missingBin");
    };
    const delta = row.amountDelta;
    const reading = {
      slot: row.unitSlot,
      stamp: row.unitStamp,
      before: format(row.before),
      after: format(row.after),
    };
    if (row.metric === "vehicles" && typeof delta === "number" && delta !== 0)
      return t(
        delta > 0
          ? "assetCapture.vehicleIncrease"
          : "assetCapture.vehicleDecrease",
        { ...reading, count: Math.abs(delta) },
      );
    return t("assetCapture.reading", reading);
  }
  const args = row.arguments as { text?: string }[] | undefined;
  return (
    args?.[0]?.text ??
    (typeof row.message === "string" ? row.message : undefined) ??
    (typeof row.text === "string" ? row.text : undefined) ??
    JSON.stringify(row.result ?? row.event ?? row)
  );
}
type LogTone =
  | "vehicles"
  | "workers"
  | "tourists"
  | "lighting"
  | "resource"
  | "simulation"
  | "engine"
  | "event"
  | "invoke"
  | "marker"
  | "unknown";
function tone(row: Record<string, unknown>): LogTone {
  const channel = String(row.channel ?? "");
  if (channel === "asset.resource") {
    switch (row.metric) {
      case "vehicles":
      case "workers":
      case "tourists":
      case "lighting":
        return row.metric;
    }
  }
  if (row.semantic === "resource_change") return "resource";
  if (row.semantic === "dispatch_event") return "event";
  if (channel === "simulation.effect" || channel === "resource.provenance")
    return "resource";
  if (channel === "simulation.rule" || channel === "simulation.condition")
    return "simulation";
  if (channel.startsWith("engine.")) return "engine";
  if (channel === "event") return "event";
  if (channel === "invoke" || channel === "result") return "invoke";
  if (channel === "host.marker") return "marker";
  return "unknown";
}
function toneLabel(value: LogTone) {
  if (
    value === "vehicles" ||
    value === "workers" ||
    value === "tourists" ||
    value === "lighting"
  )
    return t(`assetCapture.filter.${value}`);
  return t(`runtimeObserver.category.${value}`);
}
function detailFields(row: Record<string, unknown>) {
  const keys = [
    "phase",
    "ruleId",
    "ruleIndex",
    "unitSlot",
    "unitStamp",
    "simTicks",
    "gameClockSource",
    "resourceOwner",
    "operations",
    "diagnosis",
    "success",
    "name",
    "eventID",
  ];
  return keys
    .filter((key) => row[key] !== undefined)
    .slice(0, 6)
    .map((key) => [key, row[key]] as const);
}
function compact(value: unknown) {
  const serialized = typeof value === "string" ? value : JSON.stringify(value);
  return serialized.length > 180 ? `${serialized.slice(0, 177)}…` : serialized;
}
const lines = computed(() =>
  history.value
    .map((row) => ({
      row,
      text: text(row),
      serialized: JSON.stringify(row).toLowerCase(),
    }))
    .filter(
      (x) =>
        (props.scope === "ui" ||
          !props.scope ||
          x.row.observationScope === props.scope) &&
        (!props.target ||
          props.target === "all" ||
          (props.target === "school"
            ? ["school", "elementary", "小学"].some((value) =>
                x.serialized.includes(value),
              )
            : props.target === "omegaco"
              ? ["omega", "omegaco"].some((value) =>
                  x.serialized.includes(value),
                )
              : props.target === "police"
                ? ["police", "cop", "警车"].some((value) =>
                    x.serialized.includes(value),
                  )
                : x.serialized.includes(props.target.toLowerCase()))) &&
        (!props.metric || x.row.metric === props.metric) &&
        (!instance.value ||
          `${x.row.unitSlot} / ${x.row.unitStamp}` === instance.value) &&
        (!query.value.trim() ||
          `${x.text} ${lineName(x.row)} ${x.serialized}`
            .toLowerCase()
            .includes(query.value.trim().toLowerCase())),
    ),
);
const instances = computed(() => [
  ...new Set(
    history.value
      .filter((row) => row.channel === "asset.resource")
      .map((row) => `${row.unitSlot} / ${row.unitStamp}`),
  ),
]);
watch(instances, (values) => {
  if (instance.value && !values.includes(instance.value)) instance.value = "";
});
function base64Json(value: unknown) {
  const bytes = new TextEncoder().encode(JSON.stringify(value, null, 2));
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000)
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(binary);
}
async function exportLogs() {
  if (!lines.value.length) {
    toast.warning(t("runtimeObserver.noLogsToExport"));
    return;
  }
  try {
    const path = await save({
      defaultPath: "openscp-runtime-log.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    await command("write_export_file", {
      request: {
        path,
        dataBase64: base64Json(lines.value.map((line) => line.row)),
      },
    });
    toast.success(t("runtimeObserver.exportSuccess"));
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : t("runtimeObserver.exportFailed"),
    );
  }
}
function scroll() {
  const el = viewport.value;
  if (el) follow.value = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
}
async function copyLogs() {
  try {
    await navigator.clipboard.writeText(
      lines.value
        .map(
          ({ row, text }) =>
            `${timestamp(row)}  ${lineName(row) ?? ""}  ${text}`,
        )
        .join("\n"),
    );
    toast.success(t("assetCapture.copied"));
  } catch {
    toast.error(t("assetCapture.copyFailed"));
  }
}
function lineName(row: Record<string, unknown>) {
  return row.channel === "asset.resource"
    ? t(`assetCapture.resource.${row.name}`)
    : (row.name ?? row.channel);
}
function timestamp(row: Record<string, unknown>) {
  const seconds = row.gameClockSeconds;
  if (
    typeof seconds !== "number" ||
    !Number.isFinite(seconds) ||
    seconds < 0 ||
    seconds >= 86400
  )
    return "—";
  return [
    Math.floor(seconds / 3600),
    Math.floor(seconds / 60) % 60,
    Math.floor(seconds) % 60,
  ]
    .map((value) => String(value).padStart(2, "0"))
    .join(":");
}
watch([follow, lines], async () => {
  if (!follow.value) return;
  await nextTick();
  if (viewport.value) viewport.value.scrollTop = viewport.value.scrollHeight;
});
</script>
<template>
  <section class="logs">
    <header>
      <FButton
        variant="secondary"
        :disabled="!lines.length"
        class="export-button"
        @click="copyLogs"
        >{{ t("assetCapture.copy") }}</FButton
      >
      <strong>{{ t("runtimeObserver.streamTitle") }}</strong
      ><span>{{ lines.length }} {{ t("runtimeObserver.recordsShort") }}</span>
      <div v-if="instances.length > 1" class="instance-filter">
        <label :for="instanceId">{{ t("assetCapture.instance") }}</label>
        <FSelect
          :id="instanceId"
          v-model="instance"
          :options="[
            { value: '', label: t('assetCapture.allInstances') },
            ...instances.map((value) => ({ value, label: value })),
          ]"
        />
      </div>
      <label class="follow-toggle"
        ><Switch
          v-model="follow"
          :aria-label="t('runtimeObserver.follow')"
        /><span>{{ t("runtimeObserver.follow") }}</span></label
      ><FButton class="export-button" variant="secondary" @click="exportLogs">{{
        t("runtimeObserver.exportLogs")
      }}</FButton
      ><input
        v-model="query"
        type="search"
        :placeholder="t('runtimeObserver.search')"
        :aria-label="t('runtimeObserver.search')"
      />
    </header>
    <div
      ref="viewport"
      class="viewport"
      role="log"
      :aria-label="t('runtimeObserver.streamTitle')"
      tabindex="0"
      @scroll="scroll"
    >
      <p v-if="!lines.length" class="empty">
        {{
          t(
            query || (metric && history.length)
              ? "assetCapture.noMatches"
              : "runtimeObserver.noLogs",
          )
        }}
      </p>
      <div
        v-for="line in lines"
        :key="String(line.row.sequence)"
        class="line"
        tabindex="0"
        :class="[
          `tone-${tone(line.row)}`,
          { event: line.row.channel === 'event' },
        ]"
      >
        <time
          :title="
            t(
              line.row.gameClockAgeMs
                ? 'assetCapture.sampledClock'
                : 'runtimeObserver.gameClock',
            )
          "
          >{{ timestamp(line.row) }}</time
        >
        <span class="line-kind">{{ toneLabel(tone(line.row)) }}</span>
        <span class="line-name">{{ lineName(line.row) }}</span>
        <code>{{ line.text }}</code>
        <aside class="line-details" role="tooltip">
          <dl>
            <div>
              <dt>{{ t("runtimeObserver.detailCategory") }}</dt>
              <dd>{{ toneLabel(tone(line.row)) }}</dd>
            </div>
            <div>
              <dt>{{ t("runtimeObserver.detailChannel") }}</dt>
              <dd>
                {{ line.row.channel ?? t("runtimeObserver.unknownValue") }}
              </dd>
            </div>
            <div>
              <dt>{{ t("runtimeObserver.detailSequence") }}</dt>
              <dd>{{ line.row.sequence ?? "—" }}</dd>
            </div>
            <div
              v-for="([key, value], index) in detailFields(line.row)"
              :key="`${key}-${index}`"
            >
              <dt>{{ key }}</dt>
              <dd>{{ compact(value) }}</dd>
            </div>
          </dl>
        </aside>
      </div>
    </div>
  </section>
</template>
<style scoped>
.logs {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
  min-width: 0;
  background: #08090b;
  color: #f1f3f6;
}
header {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 38px;
  padding: 4px 12px;
  border-bottom: 1px solid var(--border);
  font-size: 12px;
  background: var(--surface);
}
header strong {
  font-weight: 500;
}
header span {
  color: var(--muted-foreground);
}
.follow-toggle {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
}
.instance-filter {
  display: flex;
  align-items: center;
  gap: 6px;
}
.instance-filter label {
  white-space: nowrap;
}
.follow-toggle :deep(button) {
  flex: none;
}
input[type="search"] {
  width: 190px;
  height: 28px;
  background: var(--background);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 0 8px;
  font-size: 12px;
}

.export-button {
  min-height: 28px;
  height: 28px;
  padding: 0 9px;
  border-color: var(--accent);
  background: transparent;
  color: var(--foreground);
  font-size: 11px;
  font-weight: 600;
}
.export-button:hover:not(:disabled) {
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}
.viewport {
  block-size: clamp(300px, 48vh, 620px);
  overflow: auto;
  font:
    12px/22px ui-monospace,
    Consolas,
    monospace;
  padding: 6px 0;
}
.line {
  position: relative;
  display: flex;
  gap: 12px;
  padding: 0 12px;
  min-width: max-content;
  white-space: pre;
  min-height: 22px;
}
.line time {
  color: #9ca8b8;
  flex: 0 0 8ch;
  font-variant-numeric: tabular-nums;
}
.line > span {
  color: #bdc6d3;
  overflow: hidden;
  flex: none;
}
.line-kind {
  width: 78px;
  font-weight: 600;
  letter-spacing: 0.01em;
}
.line-name {
  width: 150px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.line code {
  font: inherit;
}
.tone-vehicles {
  --log-color: #78daf4;
}
.tone-workers {
  --log-color: #8cdda4;
}
.tone-tourists {
  --log-color: #c9a8ff;
}
.tone-lighting {
  --log-color: #f8cf83;
}
.line:where(.tone-vehicles, .tone-workers, .tone-tourists, .tone-lighting) {
  box-shadow: inset 2px 0 var(--log-color);
  background: color-mix(in srgb, var(--log-color) 8%, transparent);
}
.line:where(.tone-vehicles, .tone-workers, .tone-tourists, .tone-lighting)
  .line-kind {
  color: var(--log-color);
}
.line:where(.tone-vehicles, .tone-workers, .tone-tourists, .tone-lighting):hover {
  background: color-mix(in srgb, var(--log-color) 15%, #08090b);
}
.tone-resource .line-kind,
.tone-resource code {
  color: #d6a85e;
}
.tone-simulation .line-kind,
.tone-simulation code {
  color: #7bb6f0;
}
.tone-engine .line-kind,
.tone-engine code {
  color: #e38b8b;
}
.tone-event .line-kind,
.tone-event code {
  color: #b88ae6;
}
.tone-invoke .line-kind,
.tone-invoke code {
  color: #72c7a0;
}
.tone-marker .line-kind,
.tone-marker code {
  color: #e7c875;
}
.tone-engine {
  background: #2b1818;
}
.line.event {
  background: #21182d;
}
.line:hover {
  background: #1b2029;
}
.line:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: -2px;
  z-index: 2;
}
.line-details {
  position: absolute;
  z-index: 4;
  top: calc(100% + 4px);
  left: 124px;
  display: none;
  width: min(420px, calc(100vw - 32px));
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--background);
  box-shadow: 0 10px 26px rgb(0 0 0 / 22%);
  white-space: normal;
  font: 11px/1.45 var(--font-sans);
  pointer-events: none;
}
.line:hover .line-details,
.line:focus-within .line-details,
.line:focus .line-details {
  display: block;
}
.line:nth-last-child(-n + 3) .line-details {
  top: auto;
  bottom: calc(100% + 4px);
}
.line-details dl {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: 4px 12px;
  margin: 0;
}
.line-details dl > div {
  display: contents;
}
.line-details dt {
  color: var(--muted-foreground);
}
.line-details dd {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
  color: var(--foreground);
}
.empty {
  color: #9ca8b8;
  padding: 12px;
  margin: 0;
  font-family: var(--font-sans);
}
:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: -2px;
}
@media (max-width: 600px) {
  header {
    flex-wrap: wrap;
  }
  input[type="search"] {
    width: 100%;
  }
}
</style>
