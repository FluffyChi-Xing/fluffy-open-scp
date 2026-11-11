import { flushPromises, mount } from "@vue/test-utils";
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
