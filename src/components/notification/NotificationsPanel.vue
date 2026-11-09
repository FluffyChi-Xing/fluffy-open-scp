<script setup lang="ts">
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { useBuildActivityStore } from "@/stores/buildActivity";

const { t } = useI18n();
const activity = useBuildActivityStore();
const { notices } = storeToRefs(activity);

function markAllRead() {
  activity.markAllRead();
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
        @click="markAllRead"
      >
        {{ $t("shell.markAllRead") }}
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
          <p>{{ $t(item.titleKey) }}</p>
          <small v-if="item.detail" class="notification-detail">{{
            item.detail
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
  display: flex;
  flex-direction: column;
  max-height: min(70vh, 420px);
  min-width: 280px;
}
.notifications-header {
  align-items: center;
  display: flex;
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
  overflow: auto;
  padding: 0;
}
.notification-item {
  align-items: flex-start;
  border-radius: var(--radius-sm);
  display: flex;
  gap: 10px;
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
.notification-text p {
  color: var(--foreground);
  font-size: 12px;
  line-height: 1.4;
  margin: 0;
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
  padding: 8px 10px;
}
.notifications-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  line-height: 1.4;
  margin: 0;
}
</style>
