<script setup lang="ts">
import { shallowRef, watch } from "vue";
import {
  type ScreenSpec,
  assembleGameUiHtml,
  fetchScreenStrings,
} from "@/lib/game-ui/assemble";

/**
 * 游戏 UI 重建组件：把静态资源（CSS/图片）与 locale 字符串表交给装配方法
 * `assembleGameUiHtml`，产出的完整 HTML 挂载到 iframe 沙箱中渲染。
 * 游戏 CSS 在 iframe 内独立生效，不会泄漏到应用样式。
 */
const props = defineProps<{ spec: ScreenSpec }>();

const html = shallowRef("");
const loading = shallowRef(false);

async function rebuild(spec: ScreenSpec) {
  loading.value = true;
  try {
    const strings = await fetchScreenStrings(spec.localePath);
    html.value = assembleGameUiHtml(spec, strings);
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.spec,
  (spec) => {
    if (spec) void rebuild(spec);
  },
  { immediate: true },
);
</script>

<template>
  <div class="game-ui-stage" aria-label="game ui rebuild stage">
    <iframe
      v-if="html"
      class="stage-frame"
      :srcdoc="html"
      :title="props.spec.id"
      sandbox=""
      tabindex="-1"
    />
    <div v-else class="stage-loading">{{ loading ? "…" : "" }}</div>
  </div>
</template>

<style scoped>
.game-ui-stage {
  width: 100%;
  aspect-ratio: 16 / 9;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--background);
  display: flex;
}
.stage-frame {
  flex: 1;
  border: 0;
  width: 100%;
  height: 100%;
}
.stage-loading {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--subtle-foreground);
}
</style>
