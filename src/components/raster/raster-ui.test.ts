import { flushPromises, mount, shallowMount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { i18n } from "@/locales";
import RasterSourcePanel from "./RasterSourcePanel.vue";
import RasterProfilePicker from "./RasterProfilePicker.vue";
import RasterToolbar from "./RasterToolbar.vue";
import RasterPreviewCard from "./RasterPreviewCard.vue";
import RasterLotPreview from "./RasterLotPreview.vue";
import PropertyEditorViewport from "@/pages/packages/components/property-editor/PropertyEditorViewport.vue";
import { catalogKey } from "@/lib/raster-editor/catalog";
import FSelect from "@/components/ui/FSelect.vue";
import RasterMaterialPanel from "./RasterMaterialPanel.vue";
import { useRasterSession } from "./useRasterSession";

const api = vi.hoisted(() => ({
  native: true,
  listResources: vi.fn(),
  catalog: vi.fn(),
  readSource: vi.fn(),
  readDecalDictionary: vi.fn(),
  readRgba: vi.fn(),
  readLotEditorSession: vi.fn(),
  listPropertyDocuments: vi.fn(),
}));
vi.mock("@/api", () => ({
  isTauri: () => api.native,
  tauriApi: { packages: api, raster: api },
}));
vi.mock("@/stores/gamePackages", () => ({
  useGamePackagesStore: () => ({
    opened: [{ package: { packageId: 42, path: "D:/game/Actual.package" } }],
  }),
}));
const global = { plugins: [i18n] };
afterEach(() => {
  i18n.global.locale.value = "zh-CN";
  api.native = true;
  vi.clearAllMocks();
});

describe("Raster editor regressions", () => {
  it.each([
    "decal-sign",
    "decal-graffiti",
    "decal-sdf",
    "decal-interior",
    "decal-rgba",
  ] as const)("shows only meaningful palette controls for %s", (profile) => {
    const session = useRasterSession();
    const wrapper = shallowMount(RasterMaterialPanel, {
      props: {
        profile,
        lot: session.lot.value,
        decal: session.decal.value,
        sampling: session.sampling.value,
      },
      global,
    });
    expect(wrapper.findAll('input[type="color"]')).toHaveLength(
      ["decal-interior", "decal-rgba"].includes(profile) ? 0 : 4,
    );
    wrapper.unmount();
  });
  it("reloads the catalog for every specialized profile and clears old selections", async () => {
    api.catalog.mockResolvedValue([]);
    const wrapper = shallowMount(RasterSourcePanel, {
      props: { profile: "rgba" },
      global,
    });
    await flushPromises();
    for (const profile of [
      "slot0",
      "slot1",
      "slot2",
      "slot3",
      "slot4",
      "slot5",
      "lot-mask",
      "lot-texture",
      "decal-sign",
      "decal-graffiti",
      "decal-sdf",
      "decal-interior",
      "decal-rgba",
    ] as const) {
      await wrapper.setProps({ profile });
      await flushPromises();
      expect(api.catalog).toHaveBeenLastCalledWith(42, profile);
      expect(wrapper.findAllComponents(FSelect)[1].props("modelValue")).toBe(
        "",
      );
    }
    await wrapper.setProps({ imageWidth: 256, imageHeight: 128 });
    expect(
      wrapper
        .findAll("input[type=number]")
        .map((i) => (i.element as HTMLInputElement).value),
    ).toEqual(["256", "128"]);
    wrapper.unmount();
  });
  it("loads a Lot reference with the actual cross-package mask identity", async () => {
    const tgi = { typeId: 0x2f4e681c, group: 4, instance: 7 };
    const owner = { typeId: 0x00b1b104, group: 8, instance: 9 };
    const entry = {
      packageId: 42,
      tgi,
      ownerPackageId: 99,
      owner,
      entryIndex: null,
    };
    api.catalog.mockResolvedValue([entry]);
    const wrapper = shallowMount(RasterSourcePanel, {
      props: { profile: "lot-mask" },
      global,
    });
    await flushPromises();
    wrapper
      .findAllComponents(FSelect)[1]
      .vm.$emit("update:modelValue", catalogKey(entry));
    await nextTick();
    await wrapper
      .findAll("button")
      .find((b) => b.text().includes("从选中资源"))!
      .trigger("click");
    expect(wrapper.emitted("loadLot")![0][0]).toMatchObject({
      packageId: 99,
      tgi: owner,
      lotMaskResolved: tgi,
      lotMaskPackageId: 42,
    });
    expect(api.readSource).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("lists in-memory Package resources and loads exact pixels with their source identity", async () => {
    const tgi = { typeId: 0x2f4e681c, group: 1, instance: 9 };
    const entry = {
      packageId: 42,
      tgi,
      ownerPackageId: 42,
      owner: tgi,
      entryIndex: null,
    };
    api.catalog.mockResolvedValue([entry]);
    api.readSource.mockResolvedValue({
      width: 1,
      height: 1,
      decodable: true,
      rgbaBase64: btoa(String.fromCharCode(12, 34, 56, 0)),
    });
    const wrapper = shallowMount(RasterSourcePanel, {
      props: { profile: "rgba" },
      global,
    });
    await flushPromises();
    expect(api.catalog).toHaveBeenCalledWith(42, "rgba");
    const selects = wrapper.findAllComponents(FSelect);
    expect(selects[0].props("options")).toContainEqual({
      value: "42",
      label: "Actual.package",
    });
    selects[1].vm.$emit("update:modelValue", catalogKey(entry));
    await nextTick();
    await wrapper
      .findAll("button")
      .find((b) => b.text().includes("从选中资源"))!
      .trigger("click");
    await flushPromises();
    const [image, source] = wrapper.emitted("image")![0] as [
      { pixels: Uint8ClampedArray },
      unknown,
    ];
    expect([...image.pixels]).toEqual([12, 34, 56, 0]);
    expect(source).toMatchObject({ packageId: 42, tgi });
    wrapper.unmount();
  });
  it("switches profiles and tool labels immediately when the locale changes", async () => {
    const picker = mount(RasterProfilePicker, {
      props: { modelValue: "slot2" },
      global,
    });
    const toolbar = shallowMount(RasterToolbar, {
      props: {
        tool: "brush",
        size: 1,
        color: "#ffffff",
        alpha: 255,
        channel: "rgba",
        quantized: false,
        layer: 0,
        tolerance: 0,
        parkingLength: 13,
        parkingSpacing: 6,
        mask: false,
        canUndo: false,
        canRedo: false,
        lot: false,
      },
      global,
    });
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(picker.text()).toContain("Six building slots");
    expect(picker.text()).toContain("Normal / AO");
    expect(picker.text()).not.toMatch(/[\u3400-\u9fff]/);
    expect(toolbar.text()).toContain("Brush");
    expect(toolbar.text()).not.toMatch(/[\u3400-\u9fff]/);
    picker.unmount();
    toolbar.unmount();
  });
  it("opens a fullscreen sheet and restores the trigger focus on close", async () => {
    const wrapper = mount(RasterPreviewCard, {
      props: { title: "Raw RGBA" },
      global,
      attachTo: document.body,
      slots: { default: '<img alt="pixels" />' },
    });
    await wrapper.get("button").trigger("click");
    await nextTick();
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.getAttribute("aria-label")).toBe("Raw RGBA");
    expect((dialog as HTMLElement).style.width).toBe("100vw");
    expect(dialog.querySelector("img")).not.toBeNull();
    (dialog.querySelector("button") as HTMLButtonElement).click();
    await nextTick();
    await nextTick();
    expect(document.activeElement).toBe(wrapper.get("button").element);
    wrapper.unmount();
  });
  it("passes live raw mask data and game atlases into the Property Editor renderer", async () => {
    api.readLotEditorSession.mockResolvedValue({
      lotSurfacePng: "surface",
      lotNormalAtlasPng: "normal",
      lotSize: [24, 48],
    });
    const wrapper = shallowMount(RasterLotPreview, {
      global,
      props: {
        raw: "original",
        png: "data:image/png;base64,mask",
        width: 32,
        height: 64,
        metersPerPixel: 0.75,
        colors: Array.from(
          { length: 4 },
          () => [1, 2, 3, 4] as [number, number, number, number],
        ),
        target: {
          name: "Lot",
          packageId: 42,
          tgi: { typeId: 1, group: 2, instance: 3 },
        },
        texture: false,
      },
    });
    await flushPromises();
    const viewport = wrapper.getComponent(PropertyEditorViewport);
    expect(viewport.props("renderMode")).toBe("refined");
    expect(viewport.props("groundOnly")).toBe(true);
    expect(viewport.props("lotSurfacePng")).toBe(
      "data:image/png;base64,surface",
    );
    expect(viewport.props("lotMaskRawRgba")).toBe("original");
    await wrapper.setProps({
      raw: "edited",
      colors: [
        [4, 5, 6, 7],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
      ],
    });
    expect(viewport.props("lotMaskRawRgba")).toBe("edited");
    expect(viewport.props("lotColors")[0]).toEqual([4, 5, 6, 7]);
    wrapper.unmount();
  });
});
