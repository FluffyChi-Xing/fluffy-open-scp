<script setup lang="ts">
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { useBuildActivityStore } from "@/stores/buildActivity";
import { buildQueueErrorKey } from "@/lib/buildQueueError";

const { t } = useI18n();
const activity = useBuildActivityStore();
const { notices } = storeToRefs(activity);

function formatDetail(detail: string): string {
  const key = buildQueueErrorKey(detail);
  return key ? t(key) : detail;
}

function formatTime(ms: number): string {
  try {
    return new Date(ms).toLocaleTimeString();
  } catch {
    return t("notification.time");
  }
}
</script>

<template>
  <div class="notifications-panel">
    <header class="notifications-header">
      <strong>{{ $t("notification.title") }}</strong>
      <button
        v-if="notices.length"
        type="button"
        class="notifications-action"
        @click="activity.clear()"
      >
        {{ $t("notification.clear") }}
      </button>
    </header>
    <ul v-if="notices.length" class="notifications-list">
      <li
        v-for="item in notices"
        :key="item.id"
        class="notification-item"
        :class="{ unread: !item.read, [item.tone]: true }"
      >
        <span class="notification-dot" aria-hidden="true" />
        <div class="notification-text">
          <div class="notification-heading">
            <p>{{ (item.status && (item.projectLabel || item.jobId)) || $t(item.titleKey) }}</p>
            <span v-if="item.status" class="notification-status" :data-status="item.status">
              {{ $t(`notification.buildStatus.${item.status}`) }}
            </span>
          </div>
          <progress
            v-if="item.status === 'running'"
            class="notification-progress"
            :aria-label="$t('notification.buildProgress')"
            :max="item.totalStages || 1"
            :value="item.totalStages ? item.completedStages?.length ?? 0 : undefined"
          />
          <small v-if="item.detail" class="notification-detail">{{
            formatDetail(item.detail)
          }}</small>
          <small>{{ formatTime(item.createdAtMs) }}</small>
        </div>
      </li>
    </ul>
    <div v-else class="notifications-empty">
      {{ $t("notification.empty") }}
    </div>
    <footer class="notifications-footer">
      <p class="notifications-hint">{{ $t("notification.buildHint") }}</p>
    </footer>
  </div>
</template>

<style scoped>
.notifications-panel {
  /* 高度上限交给 FPopover/useFloatingMenu；这里不再自设 overflow，避免双滚动条。 */
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  min-width: 0;
  width: 100%;
}
.notifications-header {
  align-items: center;
  display: flex;
  flex: none;
  justify-content: space-between;
  padding: 8px 10px 6px;
}
.notifications-header strong {
  color: var(--foreground);
  font-size: 13px;
  font-weight: 650;
}
.notifications-action {
  background: transparent;
  border: 0;
  color: var(--primary);
  cursor: pointer;
  font-size: 12px;
  padding: 4px 6px;
}
.notifications-action:hover {
  text-decoration: underline;
}
.notifications-list {
  list-style: none;
  margin: 0;
  padding: 0;
}
.notification-item {
  align-items: flex-start;
  border-radius: var(--radius-sm);
  display: flex;
  gap: 10px;
  min-width: 0;
  opacity: 0.72;
  padding: 9px 10px;
}
.notification-item.unread {
  opacity: 1;
}
.notification-item:hover {
  background: var(--surface-hover);
}
.notification-dot {
  background: var(--primary);
  border-radius: 50%;
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 14%, transparent);
  flex: none;
  height: 7px;
  margin-top: 5px;
  width: 7px;
}
.notification-item.success .notification-dot {
  background: var(--success, #22c55e);
  box-shadow: 0 0 0 3px color-mix(in srgb, #22c55e 14%, transparent);
}
.notification-item.error .notification-dot {
  background: var(--danger, #ef4444);
  box-shadow: 0 0 0 3px color-mix(in srgb, #ef4444 14%, transparent);
}
.notification-text {
  flex: 1;
  min-width: 0;
}
.notification-heading {
  align-items: flex-start;
  display: flex;
  gap: 6px;
  justify-content: space-between;
}
.notification-text p {
  color: var(--foreground);
  font-size: 12px;
  line-height: 1.4;
  margin: 0;
  overflow-wrap: anywhere;
}
.notification-status {
  background: var(--surface-hover);
  border-radius: 999px;
  color: var(--muted-foreground);
  flex: none;
  font-size: 10px;
  padding: 1px 6px;
}
.notification-status[data-status="running"] {
  background: color-mix(in srgb, var(--primary) 14%, transparent);
  color: var(--primary);
}
.notification-status[data-status="finished"] {
  background: color-mix(in srgb, var(--success) 14%, transparent);
  color: var(--success);
}
.notification-status[data-status="failed"] {
  background: color-mix(in srgb, var(--danger) 14%, transparent);
  color: var(--danger);
}
.notification-progress {
  accent-color: var(--primary);
  block-size: 6px;
  display: block;
  inline-size: 100%;
  margin-block: 6px;
}
.notification-text small {
  color: var(--subtle-foreground);
  display: block;
  font-size: 11px;
  margin-top: 2px;
}
.notification-detail {
  color: var(--muted-foreground);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.notifications-empty {
  color: var(--muted-foreground);
  font-size: 12px;
  padding: 22px;
  text-align: center;
}
.notifications-footer {
  border-top: 1px solid var(--border);
  flex: none;
  padding: 8px 10px;
}
.notifications-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  line-height: 1.4;
  margin: 0;
  overflow-wrap: anywhere;
}
</style>
