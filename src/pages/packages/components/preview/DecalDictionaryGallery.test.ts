import { flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { afterEach, expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import DecalDictionaryGallery from "./DecalDictionaryGallery.vue";

const source = vi.hoisted(() => ({
  readDecalDictionary: vi.fn(),
  readDecalImages: vi.fn(),
}));
vi.mock("@/api/data-source", () => ({ createDataSource: () => source }));

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it.each([
  [0x73684efc, "可悬空", "Can float"],
  [0xe5390a98, "投影", "Projected"],
  [0x4491de3a, "投影", "Projected"],
  [0x12345678, "待确认", "Unverified"],
] as const)("shows material %s placement on each item and updates the active language", async (material, zh, en) => {
  const previous = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  source.readDecalDictionary.mockResolvedValue({
    material: { instance: material },
    entries: [0, 1].map((index) => ({
      index, id: { instance: index + 1 }, rasterStatus: { decodable: true },
    })), uniformArrays: true, arrayLengths: [],
  });
  const wrapper = mount(DecalDictionaryGallery, {
    props: { packageId: 1, tgi: { typeId: 1, group: 2, instance: 3 }, compact: true, showPlacementType: true },
    global: { plugins: [i18n], stubs: { DecalImageViewerSheet: true } },
  });
  try {
    await flushPromises();
    expect(wrapper.findAll(".decal-placement-tag").map((tag) => tag.text())).toEqual([zh, zh]);
    const hint = wrapper.get(".decal-placement-tag").attributes("title");
    expect(hint).toBeTruthy();
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.findAll(".decal-placement-tag").map((tag) => tag.text())).toEqual([en, en]);
    expect(wrapper.get(".decal-placement-tag").attributes("title")).not.toBe(hint);
    await wrapper.setProps({ showPlacementType: false });
    expect(wrapper.find(".decal-placement-tag").exists()).toBe(false);
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = previous;
  }
});

it("requests only thumbnails still visible when the batch starts", async () => {
  vi.useFakeTimers();
  let intersections: IntersectionObserverCallback | undefined;
  let observerOptions: IntersectionObserverInit | undefined;
  vi.stubGlobal("IntersectionObserver", class {
    constructor(callback: IntersectionObserverCallback, options: IntersectionObserverInit) {
      intersections = callback;
      observerOptions = options;
    }
    observe() {}
    disconnect() {}
  });
  const tgi = { typeId: 1, group: 2, instance: 3 };
  const scrollRoot = document.createElement("div");
  source.readDecalDictionary.mockResolvedValue({
    entries: [0, 1, 2].map((index) => ({
      index, id: { instance: index + 1 }, rasterStatus: { decodable: true },
    })),
    uniformArrays: true,
    arrayLengths: [],
  });
  source.readDecalImages.mockResolvedValue([]);
  const wrapper = mount(DecalDictionaryGallery, {
    props: { packageId: 1, tgi, compact: true, scrollRoot },
    global: { plugins: [i18n], stubs: { DecalImageViewerSheet: true } },
  });
  try {
    await flushPromises();
    expect(source.readDecalDictionary).toHaveBeenCalledWith(1, tgi, true);
    expect(observerOptions?.root).toBe(scrollRoot);
    const cells = wrapper.findAll(".decal-cell");
    const report = (index: number, isIntersecting: boolean) => intersections!(
      [{ target: cells[index].element, isIntersecting } as IntersectionObserverEntry],
      {} as IntersectionObserver,
    );
    report(0, true);
    report(0, false);
    report(1, true);
    await vi.advanceTimersByTimeAsync(60);
    expect(source.readDecalImages).toHaveBeenCalledExactlyOnceWith(1, tgi, [1]);
  } finally {
    wrapper.unmount();
  }
});
