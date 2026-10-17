<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { ActivityEvent } from "@/api/tauri";
const props = defineProps<{ events: ActivityEvent[] }>();
const { t, locale } = useI18n();
const saved = computed(() =>
  props.events
    .filter((event) => event.topic === "runtime.key_event")
    .sort((a, b) => b.createdAt - a.createdAt),
);
function eventLabel(event: ActivityEvent) {
  const payload = event.payload as { eventID?: number } | undefined;
  return typeof payload?.eventID === "number"
    ? t("runtimeObserver.overview.event", { id: payload.eventID })
    : t("runtimeObserver.overview.unknownEvent");
}
function date(value: number) {
  return new Date(value).toLocaleString(locale.value, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}
</script>

<template>
  <section class="runtime-card" aria-labelledby="runtime-card-title">
    <div class="intro">
      <span class="icon"
        ><FIcon name="Terminal" :size="20" aria-label=""
      /></span>
      <div>
        <h2 id="runtime-card-title">
          {{ t("runtimeObserver.overview.title") }}
        </h2>
        <p>{{ t("runtimeObserver.overview.description") }}</p>
      </div>
    </div>
    <div class="saved-summary">
      <span>{{ t("runtimeObserver.overview.recent") }}</span>
      <strong>{{ saved.length.toLocaleString(locale) }}</strong>
    </div>
    <div class="latest">
      <template v-if="saved.length">
        <span class="caption">{{ t("runtimeObserver.overview.latest") }}</span>
        <strong>{{ eventLabel(saved[0]) }}</strong>
        <time :datetime="new Date(saved[0].createdAt).toISOString()">{{
          date(saved[0].createdAt)
        }}</time>
      </template>
      <template v-else>
        <strong>{{ t("runtimeObserver.overview.empty") }}</strong>
        <p>{{ t("runtimeObserver.overview.hint") }}</p>
      </template>
    </div>
    <RouterLink class="open-console" to="/runtime-observer">
      {{ t("runtimeObserver.overview.open")
      }}<FIcon name="ArrowRight" :size="15" aria-label="" />
    </RouterLink>
    <footer>
      <FIcon name="Database" :size="13" aria-label="" />{{
        t("runtimeObserver.overview.retention")
      }}
    </footer>
  </section>
</template>

<style scoped>
.runtime-card {
  display: grid;
  grid-template-columns: minmax(220px, 1.2fr) minmax(130px, 0.6fr) minmax(
      200px,
      1fr
    ) auto;
  align-items: center;
  gap: 20px;
  padding: 20px 24px 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--foreground);
  min-width: 0;
}
.intro {
  display: flex;
  gap: 12px;
  align-items: center;
}
.icon {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  flex: none;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--background);
  color: var(--primary);
}
h2 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}
p {
  margin: 5px 0 0;
  color: var(--muted-foreground);
  font-size: 12px;
  line-height: 1.6;
}
.saved-summary {
  display: grid;
  gap: 5px;
  padding-inline: 20px;
  border-inline-start: 1px solid var(--border);
}
.saved-summary span,
.caption,
time {
  color: var(--muted-foreground);
  font-size: 12px;
}
.saved-summary strong {
  font-size: 26px;
  font-weight: 600;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
}
.latest {
  display: grid;
  gap: 4px;
  min-width: 0;
}
.latest strong {
  font-size: 12px;
  font-weight: 500;
  overflow-wrap: anywhere;
}
.latest p {
  margin: 0;
}
.open-console {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  font-size: 12px;
  color: var(--foreground);
  text-decoration: none;
  white-space: nowrap;
}
.open-console:hover {
  background: var(--surface-hover);
  border-color: var(--primary);
}
.open-console:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 3px;
}
footer {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: 7px;
  border-top: 1px solid var(--border);
  padding-block: 10px;
  font-size: 11px;
  color: var(--muted-foreground);
}
@media (max-width: 1100px) {
  .runtime-card {
    grid-template-columns: 1fr 1fr;
  }
  .saved-summary {
    border: 0;
    padding: 0;
    text-align: end;
  }
  .open-console {
    justify-self: end;
  }
}
@media (max-width: 600px) {
  .runtime-card {
    grid-template-columns: 1fr;
    padding: 16px 16px 0;
    gap: 16px;
  }
  .saved-summary {
    text-align: start;
  }
  .open-console {
    justify-self: stretch;
  }
}
</style>
