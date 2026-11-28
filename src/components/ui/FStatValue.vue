<script setup lang="ts">
defineProps<{ value: string | number }>();
</script>

<template>
  <span class="stat-flip">
    <span :key="value" class="stat-flip-face">{{ value }}</span>
  </span>
</template>

<style scoped>
.stat-flip {
  display: inline-block;
  font-variant-numeric: tabular-nums;
  perspective: 10em;
  vertical-align: baseline;
}
.stat-flip-face {
  display: inline-block;
  transform-origin: center bottom;
}
@media (prefers-reduced-motion: no-preference) {
  .stat-flip-face {
    animation: stat-flip var(--motion-emphasis) var(--motion-ease-out) both;
  }
}
@keyframes stat-flip {
  from {
    opacity: 0.2;
    transform: translateY(-0.24em) rotateX(-74deg);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
</style>
