<script setup lang="ts">
interface EmblaApiType {
  canScrollPrev(): boolean;
  canScrollNext(): boolean;
  selectedScrollSnap(): number;
  scrollSnapList(): number[];
}
import { type InjectionKey, type Ref, inject, watch } from "vue";

interface CarouselContext {
  api: Ref<EmblaApiType | undefined>;
  onApiInit: (api: EmblaApiType) => void;
  onSelect: (api: EmblaApiType) => void;
}

const context = inject(
  Symbol.for("carousel-context") as InjectionKey<CarouselContext>,
);
if (!context) {
  throw new Error("CarouselContent must be used inside <Carousel>");
}
// embla 的 api ref 在挂载后填充，这里统一订阅初始化/选中事件
watch(
  () => context.api.value,
  (api) => {
    if (!api) return;
    context.onApiInit(api);
  },
  { immediate: true },
);
</script>

<template>
  <div class="carousel-track" style="display: flex">
    <slot />
  </div>
</template>
