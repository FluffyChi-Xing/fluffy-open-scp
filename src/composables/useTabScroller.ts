import {
  nextTick,
  onBeforeUnmount,
  onMounted,
  shallowRef,
  watch,
  type Ref,
} from "vue";

export function useTabScroller(content: Ref<unknown>) {
  const scroller = shallowRef<HTMLElement | null>(null);
  const canStart = shallowRef(false);
  const canEnd = shallowRef(false);
  let observer: ResizeObserver | undefined;

  function update() {
    const element = scroller.value;
    if (!element) return;
    canStart.value = element.scrollLeft > 1;
    canEnd.value =
      element.scrollLeft + element.clientWidth < element.scrollWidth - 1;
  }
  function scroll(direction: -1 | 1) {
    scroller.value?.scrollBy({ left: direction * 220, behavior: "smooth" });
  }

  onMounted(() => {
    const element = scroller.value;
    if (!element || typeof ResizeObserver === "undefined") return;
    observer = new ResizeObserver(update);
    observer.observe(element);
    void nextTick(update);
  });
  onBeforeUnmount(() => observer?.disconnect());
  watch(content, () => void nextTick(update));

  return { scroller, canStart, canEnd, scroll, update };
}
