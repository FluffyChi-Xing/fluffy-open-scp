<script setup lang="ts">
import { onMounted } from 'vue'
import { RouterView } from 'vue-router'
import { MotionConfig } from 'motion-v'
import { motionSpring } from '@/lib/motion/presets'
import FToastHost from '@/components/ui/FToastHost.vue'
import { usePressMotion } from '@/lib/motion/press'
import { useBuildActivityStore } from '@/stores/buildActivity'

/** 全局按压反馈：按钮（含 role=button）按下/松开由 motion-v 驱动。 */
usePressMotion()

onMounted(() => {
  void useBuildActivityStore().startListening()
})
</script>

<template>
  <MotionConfig reduced-motion="user" :transition="motionSpring">
    <RouterView />
    <FToastHost />
  </MotionConfig>
</template>
