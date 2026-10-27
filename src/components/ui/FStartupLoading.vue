<script setup lang="ts">
/**
 * 启动页同款加载效果（index.html #app-startup 的组件化版本）：
 * logo + 流动进度条 + 状态文案。铺满父容器，用于页面级重建占位。
 */
defineProps<{ label?: string }>();
</script>

<template>
  <div class="startup-loading" role="status" aria-live="polite">
    <img src="/scp-logo.png" width="64" height="64" alt="" />
    <strong>OpenSCP</strong>
    <div class="startup-progress" aria-hidden="true"><span></span></div>
    <span v-if="label" class="startup-label">{{ label }}</span>
  </div>
</template>

<style scoped>
.startup-loading {
  align-items: center;
  background: var(--background);
  border-radius: var(--radius-lg);
  color: var(--foreground);
  display: flex;
  flex-direction: column;
  font-family: var(--font-sans);
  gap: 20px;
  height: 100%;
  justify-content: center;
  width: 100%;
}
.startup-loading strong {
  font-size: 20px;
  font-weight: 600;
  letter-spacing: -0.03em;
}
.startup-label {
  color: var(--muted-foreground);
  font-size: 12px;
}
.startup-progress {
  background: var(--border);
  border-radius: 999px;
  height: 3px;
  overflow: hidden;
  width: 144px;
}
.startup-progress span {
  animation: f-startup-progress 1.3s ease-in-out infinite;
  background: var(--primary);
  border-radius: inherit;
  display: block;
  height: 100%;
  width: 40%;
}
@keyframes f-startup-progress {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(350%);
  }
}
@media (prefers-reduced-motion: reduce) {
  .startup-progress span {
    animation: none;
    width: 100%;
  }
}
</style>
