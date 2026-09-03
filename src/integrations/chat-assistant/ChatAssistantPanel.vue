<script setup lang="ts">
import { ref } from 'vue'
import { ChatAssistant, createGatewayTransport } from '@fluffy-design-pro/chat-assistant'
import '@fluffy-design-pro/chat-assistant/style.css'

const open = ref(false)

const transport = createGatewayTransport({
  endpoint: '/api/ai/responses'
})

const request = {
  model: 'placeholder',
  capability: 'chat'
} as const
</script>

<template>
  <ChatAssistant v-model:open="open" :transport="transport" :request="request" title="AI 助手" @error="console.error" />
</template>
