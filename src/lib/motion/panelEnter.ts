import { nextTick, watch, type Ref } from "vue";
import { useAnimate, useReducedMotion } from "motion-v";
import { motionEnter } from "./presets";

/**
 * 浮层面板入场动画（motion-v）。
 *
 * 下拉/气泡面板是 `v-if` + Teleport 结构，用内联指令在 Vue 测试环境里会
 * 触发 "Could not find original vnode" 警告；这里改成打开后对面板元素做
 * 一次命令式入场。
 */
export function usePanelEnter(
  open: Ref<boolean>,
  panel: Ref<HTMLElement | null | undefined>,
) {
  const [, animate] = useAnimate();
  const reduced = useReducedMotion();
  watch(open, async (value) => {
    if (!value || reduced.value) return;
    await nextTick();
    const element = panel.value;
    if (!element || !open.value) return;
    animate(
      element,
      { opacity: [0, 1], scale: [0.97, 1], y: [-8, 0] },
      motionEnter,
    )?.finished?.catch?.(() => {});
  });
}
