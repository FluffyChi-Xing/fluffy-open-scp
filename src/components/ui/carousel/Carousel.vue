<script setup lang="ts">
import emblaCarouselVue from "embla-carousel-vue";

/** embla 实例的最小接口（避免直接依赖 embla-carousel 包的类型路径）。 */
interface EmblaApiType {
  canScrollPrev(): boolean;
  canScrollNext(): boolean;
  selectedScrollSnap(): number;
  scrollSnapList(): number[];
  scrollPrev(): void;
  scrollNext(): void;
  on(event: string, callback: (api: EmblaApiType) => void): EmblaApiType;
}
import {
  type InjectionKey,
  type Ref,
  computed,
  provide,
  ref,
  toValue,
} from "vue";

interface CarouselContext {
  api: Ref<EmblaApiType | undefined>;
  scrollPrev: () => void;
  scrollNext: () => void;
  canScrollPrev: Ref<boolean>;
  canScrollNext: Ref<boolean>;
  selectedIndex: Ref<number>;
  scrollCount: Ref<number>;
  onApiInit: (api: EmblaApiType) => void;
  onSelect: (api: EmblaApiType) => void;
}

const props = withDefaults(
  defineProps<{
    options?: Record<string, unknown>;
    plugins?: unknown[];
  }>(),
  { options: () => ({}), plugins: () => [] },
);

const [emblaRef, emblaApi] = emblaCarouselVue(
  computed(() => ({ ...toValue(props.options) })),
  toValue(props.plugins),
);
const canScrollPrev = ref(false);
const canScrollNext = ref(false);
const selectedIndex = ref(0);
const scrollCount = ref(0);

function scrollPrev() {
  emblaApi.value?.scrollPrev();
}
function scrollNext() {
  emblaApi.value?.scrollNext();
}
const emit = defineEmits<{ select: [index: number] }>();

function onSelect(api: EmblaApiType) {
  canScrollPrev.value = api.canScrollPrev();
  canScrollNext.value = api.canScrollNext();
  selectedIndex.value = api.selectedScrollSnap();
  emit("select", selectedIndex.value);
}
function onApiInit(api: EmblaApiType) {
  scrollCount.value = api.scrollSnapList().length;
  onSelect(api);
  api.on("select", onSelect).on("reInit", (instance: EmblaApiType) => {
    scrollCount.value = instance.scrollSnapList().length;
    onSelect(instance);
  });
}

const context: CarouselContext = {
  api: emblaApi as Ref<EmblaApiType | undefined>,
  scrollPrev,
  scrollNext,
  canScrollPrev,
  canScrollNext,
  selectedIndex,
  scrollCount,
  onApiInit,
  onSelect,
};
provide(Symbol.for("carousel-context") as InjectionKey<CarouselContext>, context);

defineExpose({ api: emblaApi, scrollPrev, scrollNext });
</script>

<template>
  <div class="carousel-root" role="region" aria-roledescription="carousel">
    <div ref="emblaRef" class="carousel-viewport" style="overflow: hidden">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.carousel-root {
  position: relative;
}
</style>
