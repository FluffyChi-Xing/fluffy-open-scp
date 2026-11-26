import { flushPromises, mount, shallowMount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import { h, nextTick } from "vue";
import { i18n } from "@/locales";
import RasterSourcePanel from "./RasterSourcePanel.vue";
import RasterEditor from "./RasterEditor.vue";
import DecalTypeTabs from "./sign/DecalTypeTabs.vue";
import RasterSignControls from "./sign/RasterSignControls.vue";
import RasterDecalPreview from "./RasterDecalPreview.vue";
import GraffitiProjectionPreview from "./sign/GraffitiProjectionPreview.vue";
import RasterToolbar from "./RasterToolbar.vue";
import RasterCanvas from "./RasterCanvas.vue";
import LotProjectionOverlay from "./LotProjectionOverlay.vue";
import RasterPreviewCard from "./RasterPreviewCard.vue";
import RasterLotPreview from "./RasterLotPreview.vue";
import LotEdgeControls from "./LotEdgeControls.vue";
import PropertyEditorViewport from "@/pages/packages/components/property-editor/PropertyEditorViewport.vue";
import { catalogKey } from "@/lib/raster-editor/catalog";
import FSelect from "@/components/ui/FSelect.vue";
import { Switch } from "@/components/ui/switch";
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
  readLotPreviewAtlases: vi.fn().mockResolvedValue(null),
  listPropertyDocuments: vi.fn(),
}));
vi.mock("@/api", () => ({
  isTauri: () => api.native,
  tauriApi: { packages: api, raster: api },
}));
vi.mock("@/composables/useEditorPackages", async original => ({
  ...await original<typeof import("@/composables/useEditorPackages")>(),
  useEditorPackageContext: () => ({
    opened: [{ package: { packageId: 42, path: "D:/game/Actual.package" } }],
  }),
}));
const global = { plugins: [i18n] };
afterEach(() => {
  vi.unstubAllGlobals();
  i18n.global.locale.value = "zh-CN";
  api.native = true;
  vi.clearAllMocks();
});

describe("Raster editor regressions", () => {
  it("paints hollow and filled Lot circles using the radius in the right inspector", async () => {
    const getContext = vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    const panel = shallowMount(RasterEditor, {
      props: { initialProfile: "lot-mask", initialLotSize: [18, 18] },
      global: {
        ...global,
        stubs: {
          RasterCanvas: false,
          RasterToolbar: false,
          FTooltip: { props: ["text"], template: '<slot name="trigger" />' },
        },
      },
    });
    try {
      const canvas = panel.getComponent(RasterCanvas);
      const surface = canvas.get("canvas.surface");
      Object.defineProperties(canvas.element, {
        clientWidth: { value: 72 },
        clientHeight: { value: 72 },
        setPointerCapture: { value: () => {} },
        hasPointerCapture: { value: () => false },
      });
      canvas.vm.fit();
      await nextTick();
      const controls = panel.get(".inspector .editor-controls");
      expect(panel.get(".inspector").element.firstElementChild).toBe(controls.element);
      const toolbar = panel.getComponent(RasterToolbar);
      expect(controls.text()).not.toContain("半径");
      await toolbar.get('button[aria-label="圆形"]').trigger("click");
      expect(controls.text()).toContain("半径（像素）");
      await controls.get<HTMLInputElement>("input").setValue("3");
      const pointer = { clientX: 36, clientY: 36, button: 0, pointerId: 1 };
      await surface.trigger("pointerdown", pointer);
      await surface.trigger("pointerup", pointer);
      const doc = canvas.props("doc");
      expect(doc.getPixel(12, 11)).toEqual([0, 0, 0, 0]);
      expect(doc.getPixel(15, 11)).toEqual(canvas.props("color"));
      expect(doc.getPixel(16, 11)).toEqual([0, 0, 0, 0]);

      i18n.global.locale.value = "en-US";
      await nextTick();
      await toolbar.get('button[aria-label="Filled circle"]').trigger("click");
      expect(controls.text()).toContain("Radius (px)");
      expect(controls.findAll("input")).toHaveLength(1);
      await controls.get<HTMLInputElement>("input").setValue("5");
      await surface.trigger("pointerdown", pointer);
      await surface.trigger("pointerup", pointer);
      expect(doc.getPixel(12, 11)).toEqual(canvas.props("color"));
      expect(doc.getPixel(17, 11)).toEqual(canvas.props("color"));
      expect(doc.getPixel(18, 11)).toEqual([0, 0, 0, 0]);
      await toolbar.get('button[aria-label="Undo"]').trigger("click");
      expect(doc.getPixel(12, 11)).toEqual([0, 0, 0, 0]);
      expect(doc.getPixel(15, 11)).toEqual(canvas.props("color"));
    } finally {
      panel.unmount();
      getContext.mockRestore();
    }
  });

  it("starts a rectangular lot at workbench size with a localized model guide", async () => {
    const projection = {
      payload: { glbs: [], materials: [], meshMaterialIndices: [], meshUvKinds: [], diagnostics: "" },
      origin: null,
      transform: null,
      center: null,
    };
    const wrapper = shallowMount(RasterEditor, {
      props: { initialProfile: "lot-mask", initialLotSize: [90, 203], lotProjection: projection },
      global: { ...global, renderStubDefaultSlot: true },
    });
    const canvas = wrapper.getComponent(RasterCanvas);
    expect(canvas.props("doc")).toMatchObject({ width: 120, height: 271 });
    expect(wrapper.vm.snapshot()?.lot.size).toEqual([90, 203]);
    const before = wrapper.vm.snapshot();
    const guide = wrapper.getComponent(LotProjectionOverlay);
    expect(guide.props("size")).toEqual([90, 203]);
    expect(wrapper.get(".lot-projection-toggle").text()).toContain("建筑俯视投影");
    wrapper.getComponent(Switch).vm.$emit("update:modelValue", false);
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.getComponent(Switch).props("modelValue")).toBe(false);
    expect(guide.attributes("style")).toContain("display: none");
    expect(wrapper.get(".lot-projection-toggle").text()).toContain("Building projection");
    expect(wrapper.vm.snapshot()).toEqual(before);
    wrapper.unmount();
  });

  it("edits the selected curb channel and relabels it when the locale changes", async () => {
    const { lot } = useRasterSession();
    const wrapper = mount(LotEdgeControls, {
      global,
      props: { modelValue: lot.value, slot: 2 },
    });
    i18n.global.locale.value = "zh-CN";
    await nextTick();
    expect(wrapper.text()).toContain("边框高度");
    const width = wrapper.get<HTMLInputElement>('input[type="range"]');
    expect(width.attributes()).toMatchObject({ min: "0", max: "0.5", step: "0.001" });
    width.element.value = "0.353";
    await width.trigger("input");
    expect(width.element.valueAsNumber).toBe(0.353);
    expect(wrapper.get(".lot-edge-width-value").text()).toBe("0.353");
    expect(wrapper.emitted("update:modelValue")).toBeUndefined();
    await width.trigger("change");
    expect(wrapper.emitted("update:modelValue")?.[0]?.[0]).toMatchObject({
      borderWidths: [0, 0, 0.353, 0],
    });
    await wrapper.findAll('input[type="number"]')[1].setValue("-2.5");
    expect(wrapper.emitted("update:modelValue")?.[1]?.[0]).toMatchObject({
      borderHeights: [0, 0, -2.5, 0],
    });
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("Border height");
    wrapper.unmount();
  });

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
  it("offers only ground and decal painting and translates the active panel live", async () => {
    const panel = shallowMount(RasterEditor, {
      global: { ...global, stubs: { DecalTypeTabs: false, RasterSignControls: false } },
    });
    const select = panel.getComponent(FSelect);
    expect(select.props("options")).toEqual([
      { value: "", label: "选择 Raster 类型以开始" },
      { value: "lot-mask", label: "地面绘制" },
      { value: "decal-sign", label: "贴花绘制" },
      { value: "decal-graffiti", label: "涂鸦" },
    ]);
    select.vm.$emit("update:modelValue", "lot-mask");
    await nextTick();
    expect(panel.getComponent(RasterSourcePanel).props("profile")).toBe("lot-mask");
    select.vm.$emit("update:modelValue", "decal-sign");
    await nextTick();
    expect(panel.getComponent(RasterSourcePanel).props("profile")).toBe("decal-sign");
    const tabs = panel.getComponent(DecalTypeTabs);
    expect(tabs.get('[aria-selected="true"]').text()).toBe("招牌 / 广告牌");
    expect(tabs.findAll('button[role="tab"]:not([disabled])')).toHaveLength(2);
    expect(tabs.findAll('button[role="tab"][disabled]')).toHaveLength(5);
    await panel.getComponent(RasterSignControls).findAll('input[type="radio"]')[1].setValue();
    expect(panel.text()).toContain("扫光速度");
    await panel.get(".sign-start button").trigger("click");
    panel.getComponent(RasterToolbar).vm.$emit("update:tool", "fill");
    await nextTick();
    expect(panel.get(".editor-controls").text()).toContain("穿过锁定轮廓");

    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(select.props("options")).toEqual([
      { value: "lot-mask", label: "Ground painting" },
      { value: "decal-sign", label: "Decal painting" },
      { value: "decal-graffiti", label: "Graffiti" },
    ]);
    expect(tabs.get('[aria-selected="true"]').text()).toBe("Signs and billboards");
    expect(panel.get(".sign-properties-toggle").text()).toBe("Properties");
    expect(panel.get(".sign-document-settings summary").text()).toContain("Decal details");
    expect(panel.get(".sign-canvas-settings summary").text()).toContain("Canvas settings");
    expect(panel.text()).toContain("Sweep speed");
    expect(panel.get(".editor-controls").text()).toContain("Fill through locked outlines");
    panel.unmount();
  });
  it("switches to graffiti and edits paint strength without changing the sign material", async () => {
    const panel = shallowMount(RasterEditor, {
      props: { initialProfile: "decal-sign" },
      global: { ...global, stubs: { DecalTypeTabs: false, RasterSignControls: false } },
    });
    await panel.getComponent(DecalTypeTabs).get('[aria-selected="true"]').trigger("keydown", { key: "ArrowRight" });
    expect(panel.getComponent(DecalTypeTabs).get('[aria-selected="true"]').text()).toBe("涂鸦");
    await panel.get(".sign-start button").trigger("click");
    const controls = panel.getComponent(RasterSignControls);
    expect(controls.find('input[type="radio"]').exists()).toBe(false);
    await controls.get('input[type="range"]').setValue("180");
    await controls.findAll(".palette-row button")[3].trigger("click");
    expect(panel.getComponent(RasterCanvas).props("color")).toEqual([0, 0, 0, 180]);
    panel.getComponent(RasterCanvas).vm.$emit("pick-color", [210, 0, 0, 0]);
    await nextTick();
    expect(panel.getComponent(RasterCanvas).props("color")).toEqual([210, 0, 0, 0]);
    expect(panel.getComponent(GraffitiProjectionPreview).props("aspectRatio")).toBe(2);
    expect(panel.findComponent(RasterDecalPreview).exists()).toBe(false);
    const snapshot = panel.vm.snapshot()!;
    expect(snapshot).toMatchObject({ profile: "decal-graffiti", image: { width: 128, height: 64 } });
    expect(snapshot.decal.sign).toBeUndefined();
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(controls.text()).toContain("Paint strength");
    const restored = shallowMount(RasterEditor, { props: { initialSchema: snapshot }, global });
    expect(restored.getComponent(GraffitiProjectionPreview).props("aspectRatio")).toBe(2);
    restored.unmount();
    panel.unmount();
  });
  it("rejects an unsupported document in the panel and accepts a decal document", async () => {
    const panel = shallowMount(RasterEditor, { global });
    panel.getComponent(FSelect).vm.$emit("update:modelValue", "lot-mask");
    await nextTick();
    const source = panel.getComponent(RasterSourcePanel);
    const session = useRasterSession();
    session.changeProfile("slot0");
    source.vm.$emit("schema", session.snapshot());
    await nextTick();
    expect(source.props("profile")).toBe("lot-mask");
    expect(panel.get('[role="alert"]').text()).toContain("贴花绘制");

    session.changeProfile("decal-sign");
    session.create(1, 1);
    source.vm.$emit("schema", session.snapshot());
    await nextTick();
    expect(panel.getComponent(RasterSourcePanel).props("profile")).toBe("decal-sign");
    expect(panel.find('[role="alert"]').exists()).toBe(false);
    panel.unmount();
  });
  it("creates a sign preset and reopens its marquee settings", async () => {
    const panel = shallowMount(RasterEditor, { global });
    panel.getComponent(FSelect).vm.$emit("update:modelValue", "decal-sign");
    await nextTick();
    const inspector = panel.get<HTMLElement>(".sign-inspector");
    expect(panel.find(".left-rail").exists()).toBe(false);
    expect(inspector.get(".sign-document-settings").attributes()).not.toHaveProperty("open");
    expect(inspector.findComponent(DecalTypeTabs).exists()).toBe(true);
    expect(panel.get(".workspace").findComponent(DecalTypeTabs).exists()).toBe(false);
    expect(inspector.get(".sign-canvas-settings").attributes()).toHaveProperty("open");
    expect(inspector.findComponent(RasterSourcePanel).exists()).toBe(true);
    await panel.get(".sign-start button").trigger("click");
    expect(inspector.get(".sign-canvas-settings").attributes()).not.toHaveProperty("open");
    const settings = { effect: "marquee", axis: "vertical", speed: 0.6 } as const;
    panel.getComponent(RasterSignControls).vm.$emit("settings", settings);
    await nextTick();
    expect(panel.getComponent(RasterDecalPreview).props("sign")).toEqual(settings);
    const properties = panel.get(".sign-properties-toggle");
    await properties.trigger("click");
    expect(properties.attributes("aria-expanded")).toBe("false");
    expect(inspector.element.style.display).toBe("none");
    expect(panel.findComponent(RasterToolbar).exists()).toBe(true);
    await properties.trigger("click");
    expect(properties.attributes("aria-expanded")).toBe("true");
    expect(inspector.element.style.display).not.toBe("none");
    expect(panel.getComponent(RasterSignControls).props("settings")).toEqual(settings);
    const snapshot = panel.vm.snapshot();
    expect(snapshot?.image).toMatchObject({ width: 256, height: 128 });
    expect(snapshot?.decal.sign).toEqual(settings);
    panel.unmount();

    const reopened = shallowMount(RasterEditor, { props: { initialSchema: snapshot! }, global });
    expect(reopened.getComponent(RasterSignControls).props("settings")).toEqual(settings);
    expect(reopened.find(".material-actions").exists()).toBe(false);
    reopened.unmount();
  });
  it("keeps the embedded editor on its assigned material slot", async () => {
    const panel = shallowMount(RasterEditor, {
      props: { mode: "flow", initialProfile: "slot0", lockedProfile: true },
      global,
    });
    const select = panel.getComponent(FSelect);
    expect(select.props("disabled")).toBe(true);
    expect(select.props("options").map(({ value }) => value)).toEqual(["slot0"]);
    select.vm.$emit("update:modelValue", "decal-sign");
    await nextTick();
    expect(panel.getComponent(RasterSourcePanel).props("profile")).toBe("slot0");
    panel.unmount();
  });
  it("switches tool labels immediately when the locale changes", async () => {
    const toolbar = mount(RasterToolbar, {
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
    expect(toolbar.find('button[aria-label="圆形"]').exists()).toBe(true);
    expect(toolbar.find('button[aria-label="实心圆形"]').exists()).toBe(true);
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(toolbar.findAll("button[aria-label]").map(b => b.attributes("aria-label"))).toContain("Brush");
    await toolbar.get('button[aria-label="Circle"]').trigger("click");
    await toolbar.get('button[aria-label="Filled circle"]').trigger("click");
    expect(toolbar.emitted("update:tool")).toEqual([["circle"], ["circle-fill"]]);
    expect(toolbar.text()).not.toMatch(/[\u3400-\u9fff]/);
    toolbar.unmount();
  });
  it("translates the lot drawing and layer selection controls live", async () => {
    const toolbar = mount(RasterToolbar, {
      props: {
        tool: "brush", size: 4, color: "#ffffff", alpha: 255,
        channel: "rgba", quantized: true, layer: 0, tolerance: 0,
        parkingLength: 13, parkingSpacing: 6, mask: true,
        canUndo: false, canRedo: false, lot: true, profile: "lot-mask",
      },
      global,
    });
    expect(toolbar.findAll("button[aria-label]").map(b => b.attributes("aria-label"))).toContain("点选图层");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(toolbar.findAll("button[aria-label]").map(b => b.attributes("aria-label"))).toContain("Select layer");
    expect(toolbar.findAll("button[aria-label]").map(b => b.attributes("aria-label"))).toContain("Filled polygon");
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
  it.each(["surface", null])("keeps the game-material viewport in fullscreen and back with atlas %s", async (atlas) => {
    api.readLotEditorSession.mockResolvedValue({ lotSurfacePng: atlas });
    api.readLotPreviewAtlases.mockResolvedValue(null);
    api.listPropertyDocuments.mockResolvedValue([]);
    vi.stubGlobal("IntersectionObserver", class {
      constructor(private callback: IntersectionObserverCallback) {}
      observe(target: Element) {
        this.callback([{ isIntersecting: false, target } as IntersectionObserverEntry], this as unknown as IntersectionObserver);
      }
      disconnect() {}
    });
    const wrapper = mount(RasterPreviewCard, {
      props: { title: "Lot game materials" },
      global: { ...global, stubs: { PropertyEditorViewport: true } },
      attachTo: document.body,
      slots: {
        default: () => h(RasterLotPreview, {
          raw: "mask",
          png: "data:image/png;base64,mask",
          width: 32,
          height: 32,
          metersPerPixel: 0.75,
          colors: [],
          target: { name: "Lot", packageId: 42, tgi: { typeId: 1, group: 2, instance: 3 } },
          texture: false,
          eager: true,
          renderMode: "refined",
        }),
      },
    });
    try {
      await flushPromises();
      const preview = wrapper.getComponent(PropertyEditorViewport);
      const viewport = preview.element;
      expect(preview.props("lotSurfacePng")).toBe(atlas ? "data:image/png;base64,surface" : null);
      expect(preview.props("lotMaskRawRgba")).toBe("mask");
      const reads = api.readLotEditorSession.mock.calls.length;
      expect(reads).toBeGreaterThan(0);
      const trigger = wrapper.get("button");
      await trigger.trigger("click");
      await nextTick();
      const dialog = document.querySelector('[role="dialog"][aria-label="Lot game materials"]')!;
      expect(dialog.contains(viewport)).toBe(true);
      expect(api.readLotEditorSession).toHaveBeenCalledTimes(reads);
      dialog.querySelector("button")!.click();
      await nextTick();
      expect(wrapper.element.contains(viewport)).toBe(true);
      expect(api.readLotEditorSession).toHaveBeenCalledTimes(reads);
    } finally {
      wrapper.unmount();
    }
  });
  it("passes live raw mask data and game atlases into the Property Editor renderer", async () => {
    vi.stubGlobal("IntersectionObserver", class {
      constructor(private callback: IntersectionObserverCallback) {}
      observe(target: Element) {
        this.callback([{ isIntersecting: true, target } as IntersectionObserverEntry], this as unknown as IntersectionObserver);
      }
      disconnect() {}
    });
    api.readLotEditorSession.mockResolvedValue({
      lotSurfacePng: "surface",
      lotNormalAtlasPng: "normal",
      lotSize: [24, 48],
    });
    const wrapper = shallowMount(RasterLotPreview, {
      global,
      props: {
        raw: "original",
        renderMode: "refined",
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
