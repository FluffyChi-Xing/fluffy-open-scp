<script setup lang="ts">
import { defineAsyncComponent } from 'vue'

// 预留：AI 聊天助手。网关（SSE）就绪后在 .env 设置 VITE_CHAT_ASSISTANT=true 启用。
// Tauri 下网关可由 Rust 侧本地服务或自定义协议承载，endpoint 届时再接入。
const ChatAssistantPanel = defineAsyncComponent(() => import('./ChatAssistantPanel.vue'))

const enabled = import.meta.env.VITE_CHAT_ASSISTANT === 'true'
</script>

<template>
  <ChatAssistantPanel v-if="enabled" />
</template>
