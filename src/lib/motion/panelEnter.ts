import { nextTick, watch, type Ref } from "vue";
import { useAnimate, useReducedMotion } from "motion-v";

/**
 * 浮层面板入场动画（motion-v）。
 *
 * 下拉/气泡面板是 `v-if` + Teleport 结构，用内联指令在 Vue 测试环境里会
 * 触发 "Could not find original vnode" 警告；这里改成打开后对面板元素做
 * 一次命令式 spring 入场，行为一致且无副作用。
 */
export function usePanelEnter(
  open: Ref<boolean>,
  panel: Ref<HTMLElement | null | undefined>,
  options: { scaleFrom?: number; yFrom?: number; stiffness?: number; damping?: number } = {},
) {
  const [, animate] = useAnimate();
  const reduced = useReducedMotion();
  const { scaleFrom = 0.96, yFrom = -4, stiffness = 520, damping = 34 } = options;

  watch(open, async (value) => {
    if (!value || reduced.value) return;
    await nextTick();
    const element = panel.value;
    if (!element) return;
    animate(
      element,
      { opacity: [0, 1], scale: [scaleFrom, 1], y: [yFrom, 0] },
      { type: "spring", stiffness, damping, mass: 0.7 },
    )?.finished?.catch?.(() => {});
  });
}
