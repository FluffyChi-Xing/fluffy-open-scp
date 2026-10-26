<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Switch from "@/components/ui/switch/Switch.vue";
const props = defineProps<{ rows: Record<string, unknown>[]; scope?: "ui" | "building" | "agent"; target?: string }>();
const { t, locale } = useI18n();
const query = ref("");
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
    if (follow.value)
      void nextTick(() => {
        if (viewport.value)
          viewport.value.scrollTop = viewport.value.scrollHeight;
      });
  },
  { immediate: true },
);
function text(row: Record<string, unknown>) {
  const args = row.arguments as { text?: string }[] | undefined;
  return args?.[0]?.text ?? JSON.stringify(row.result ?? row.event ?? row);
}
type LogTone =
  | "resource"
  | "simulation"
  | "engine"
  | "event"
  | "invoke"
  | "marker"
  | "unknown";
function tone(row: Record<string, unknown>): LogTone {
  const channel = String(row.channel ?? "");
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
  return t(`runtimeObserver.category.${value}`);
}
function detailFields(row: Record<string, unknown>) {
  const keys = [
    "phase",
    "ruleId",
    "ruleIndex",
    "unitSlot",
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
    .map((row) => ({ row, text: text(row), serialized: JSON.stringify(row).toLowerCase() }))
    .filter(
      (x) =>
        (props.scope === "ui" || !props.scope || x.row.observationScope === props.scope) &&
        (!props.target || props.target === "all" || (props.target === "school" ? ["school", "elementary", "小学"].some((value) => x.serialized.includes(value)) : props.target === "omegaco" ? ["omega", "omegaco"].some((value) => x.serialized.includes(value)) : props.target === "police" ? ["police", "cop", "警车"].some((value) => x.serialized.includes(value)) : x.serialized.includes(props.target.toLowerCase()))) &&
        (!query.value || x.text.toLowerCase().includes(query.value.toLowerCase())),
    ),
);
function scroll() {
  const el = viewport.value;
  if (el) follow.value = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
}
function timestamp(row: Record<string, unknown>) {
  return new Date(Number(row.time)).toLocaleTimeString(locale.value, {
    hour12: false,
    fractionalSecondDigits: 3,
  });
}
</script>
<template>
  <section class="logs">
    <header>
      <strong>{{ t("runtimeObserver.streamTitle") }}</strong
      ><span>{{ lines.length }} {{ t("runtimeObserver.recordsShort") }}</span
      ><label class="follow-toggle"
        ><Switch v-model="follow" :aria-label="t('runtimeObserver.follow')" /><span>{{
          t("runtimeObserver.follow")
        }}</span></label
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
        {{ t("runtimeObserver.noLogs") }}
      </p>
      <div
        v-for="line in lines"
        :key="String(line.row.sequence)"
        class="line"
        tabindex="0"
        :class="[`tone-${tone(line.row)}`, { event: line.row.channel === 'event' }]"
      >
        <time>{{ timestamp(line.row) }}</time>
        <span class="line-kind">{{ toneLabel(tone(line.row)) }}</span>
        <span class="line-name">{{ line.row.name ?? line.row.channel }}</span>
        <code>{{ line.text }}</code>
        <aside class="line-details" role="tooltip">
          <dl>
            <div>
              <dt>{{ t("runtimeObserver.detailCategory") }}</dt>
              <dd>{{ toneLabel(tone(line.row)) }}</dd>
            </div>
            <div>
              <dt>{{ t("runtimeObserver.detailChannel") }}</dt>
              <dd>{{ line.row.channel ?? t("runtimeObserver.unknownValue") }}</dd>
            </div>
            <div>
              <dt>{{ t("runtimeObserver.detailSequence") }}</dt>
              <dd>{{ line.row.sequence ?? "—" }}</dd>
            </div>
            <div v-for="([key, value], index) in detailFields(line.row)" :key="`${key}-${index}`">
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
  background: var(--background);
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
label {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
}
.follow-toggle :deep(button) { flex: none; }
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
.viewport {
  height: clamp(180px, 36vh, 420px);
  overflow: auto;
  font:
    12px/22px ui-monospace,
    Consolas,
    monospace;
  padding: 6px 0;
}
.viewport:has(.empty) {
  height: 80px;
}
.line {
  position: relative;
  display: flex;
  gap: 14px;
  padding: 0 12px;
  min-width: max-content;
  white-space: pre;
  min-height: 22px;
}
.line time {
  color: var(--subtle-foreground);
  flex: none;
}
.line > span {
  color: var(--muted-foreground);
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
.line.event {
  background: color-mix(in srgb, var(--accent) 45%, transparent);
}
.line:hover {
  background: var(--surface-hover);
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
  color: var(--muted-foreground);
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
