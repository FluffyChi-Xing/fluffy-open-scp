<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
const props = defineProps<{ rows: Record<string, unknown>[] }>();
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
const lines = computed(() =>
  history.value
    .map((row) => ({ row, text: text(row) }))
    .filter(
      (x) =>
        !query.value ||
        x.text.toLowerCase().includes(query.value.toLowerCase()),
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
      ><label
        ><input v-model="follow" type="checkbox" />{{
          t("runtimeObserver.follow")
        }}</label
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
        :class="{ event: line.row.channel === 'event' }"
      >
        <time>{{ timestamp(line.row) }}</time
        ><span>{{ line.row.name }}</span
        ><code>{{ line.text }}</code>
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
  height: clamp(320px, calc(100vh - 360px), 850px);
  overflow: auto;
  font:
    12px/22px ui-monospace,
    Consolas,
    monospace;
  padding: 6px 0;
}
.line {
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
  width: 180px;
  overflow: hidden;
  flex: none;
}
.line code {
  font: inherit;
}
.line.event {
  background: var(--accent);
}
.line:hover {
  background: var(--surface-hover);
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
