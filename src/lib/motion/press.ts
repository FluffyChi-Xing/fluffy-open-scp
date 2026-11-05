import { onBeforeUnmount, onMounted } from "vue";
import { useAnimate, useReducedMotion } from "motion-v";

/**
 * 全局按压反馈（motion-v）。
 *
 * 用事件委托监听整个文档，因此不需要在几百个 `<button>` 上逐个加指令：
 * 指针按下把元素缩到 0.965，松开用 spring 弹回。只作用于可交互元素，
 * 尊重 `prefers-reduced-motion`，并可通过 `data-press="off"` 退出。
 *
 * 纯表现层：不改变任何点击行为，也不阻止默认事件。
 */

const INTERACTIVE = "button, [role='button'], [data-press]";
const OPT_OUT = "[data-press='off']";
const PRESS_SCALE = 0.965;

/** 事件目标 → 需要做按压反馈的元素（纯函数，单测覆盖）。 */
export function resolvePressTarget(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof Element)) return null;
  const element = target.closest<HTMLElement>(INTERACTIVE);
  if (!element || element.closest(OPT_OUT)) return null;
  if (
    element.hasAttribute("disabled") ||
    element.getAttribute("aria-disabled") === "true"
  ) {
    return null;
  }
  return element;
}

export function usePressMotion() {
  const [, animate] = useAnimate();
  const reduced = useReducedMotion();
  let pressed: HTMLElement | null = null;

  function press(element: HTMLElement) {
    if (reduced.value) return;
    if (pressed && pressed !== element) release();
    pressed = element;
    animate(
      element,
      { scale: PRESS_SCALE },
      { duration: 0.09, ease: [0.2, 0, 0, 1] },
    ).finished?.catch?.(() => {});
  }

  function release() {
    const element = pressed;
    pressed = null;
    if (!element || reduced.value) return;
    animate(
      element,
      { scale: 1 },
      { type: "spring", stiffness: 520, damping: 26, mass: 0.5 },
    ).finished?.catch?.(() => {});
  }

  function onPointerDown(event: PointerEvent) {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    const element = resolvePressTarget(event.target);
    if (element) press(element);
  }
  function onKeyDown(event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    const element = resolvePressTarget(event.target);
    if (element) press(element);
  }
  function onKeyUp(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === " ") release();
  }

  onMounted(() => {
    document.addEventListener("pointerdown", onPointerDown, true);
    document.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("pointerup", release, true);
    window.addEventListener("pointercancel", release, true);
    window.addEventListener("keyup", onKeyUp, true);
    window.addEventListener("blur", release);
  });
  onBeforeUnmount(() => {
    document.removeEventListener("pointerdown", onPointerDown, true);
    document.removeEventListener("keydown", onKeyDown, true);
    window.removeEventListener("pointerup", release, true);
    window.removeEventListener("pointercancel", release, true);
    window.removeEventListener("keyup", onKeyUp, true);
    window.removeEventListener("blur", release);
  });
}
