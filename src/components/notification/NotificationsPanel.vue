<script setup lang="ts">
import { shallowRef } from 'vue'

interface DemoNotification { id: number; titleKey: string; timeKey: string }
const notifications = shallowRef<DemoNotification[]>([
  { id: 1, titleKey: 'home.deploymentCompleted', timeKey: 'notification.time' },
  { id: 2, titleKey: 'home.projectCreated', timeKey: 'notification.time' },
  { id: 3, titleKey: 'home.memberJoined', timeKey: 'notification.time' }
])
function markAllRead() {
  notifications.value = []
}
</script>

<template><div class="notifications-panel"><header class="notifications-header"><strong>{{ $t('notification.title') }}</strong><button v-if="notifications.length" type="button" class="notifications-action" @click="markAllRead">{{ $t('shell.markAllRead') }}</button></header><ul v-if="notifications.length" class="notifications-list"><li v-for="item in notifications" :key="item.id" class="notification-item"><span class="notification-dot" aria-hidden="true" /><div class="notification-text"><p>{{ $t(item.titleKey) }}</p><small>{{ $t(item.timeKey) }}</small></div></li></ul><div v-else class="notifications-empty">{{ $t('notification.empty') }}</div><footer class="notifications-footer"><button type="button" class="notifications-action">{{ $t('notification.viewAll') }}</button></footer></div></template>

<style scoped>
.notifications-panel{display:flex;flex-direction:column}.notifications-header{align-items:center;display:flex;justify-content:space-between;padding:8px 10px 6px}.notifications-header strong{color:var(--foreground);font-size:13px;font-weight:650}.notifications-action{background:transparent;border:0;color:var(--primary);cursor:pointer;font-size:12px;padding:4px 6px}.notifications-action:hover{text-decoration:underline}.notifications-list{list-style:none;margin:0;padding:0}.notification-item{align-items:flex-start;border-radius:var(--radius-sm);display:flex;gap:10px;padding:9px 10px}.notification-item:hover{background:var(--surface-hover)}.notification-dot{background:var(--primary);border-radius:50%;box-shadow:0 0 0 3px color-mix(in srgb,var(--primary) 14%,transparent);flex:none;height:7px;margin-top:5px;width:7px}.notification-text p{color:var(--foreground);font-size:12px;line-height:1.4;margin:0}.notification-text small{color:var(--subtle-foreground);font-size:11px}.notifications-empty{color:var(--muted-foreground);font-size:12px;padding:22px;text-align:center}.notifications-footer{border-top:1px solid var(--border);padding:6px 8px}.notifications-footer .notifications-action{color:var(--muted-foreground);text-align:center;width:100%}.notifications-footer .notifications-action:hover{color:var(--foreground);text-decoration:none}
</style>
