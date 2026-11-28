import { flushPromises, shallowMount } from "@vue/test-utils";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { i18n } from "@/locales";
import FSheet from "@/components/ui/FSheet.vue";
import FStartupLoading from "@/components/ui/FStartupLoading.vue";
import RasterEditor from "@/components/raster/RasterEditor.vue";
import { useRasterSession } from "@/components/raster/useRasterSession";
import { authoredFromSchema } from "@/lib/lot-ground/authored";
import AssetAuthoredLotSection from "./AssetAuthoredLotSection.vue";
import { loadAuthoredLot, saveAuthoredLot } from "./authoredLotStore";

vi.mock("@/api", () => ({ isTauri: () => true, tauriApi: {} }));
vi.mock("./authoredLotStore", () => ({
  loadAuthoredLot: vi.fn(),
  saveAuthoredLot: vi.fn(),
}));

const frames = new Map<number, FrameRequestCallback>();
let frameId = 0;
beforeEach(() => {
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    frames.set(++frameId, callback);
    return frameId;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
});
afterEach(() => {
  vi.unstubAllGlobals();
  frames.clear();
  vi.clearAllMocks();
});

async function advanceFrame() {
  const callbacks = [...frames.values()];
  frames.clear();
  callbacks.forEach((callback) => callback(0));
  await nextTick();
}

function mountSection(lotSize: [number, number] | null) {
  return shallowMount(AssetAuthoredLotSection, {
    props: { project: "lot-project", editEnabled: true, editMode: "draft", bound: false, lotSize },
    global: { plugins: [i18n], renderStubDefaultSlot: true },
  });
}

it("gives a new drawing the current workbench dimensions", async () => {
  vi.mocked(loadAuthoredLot).mockResolvedValue(null);
  const wrapper = mountSection([90, 203]);
  try {
    await flushPromises();
    await wrapper.get(".draw-stage").trigger("click");
    expect(wrapper.getComponent(FSheet).props("open")).toBe(true);
    expect(wrapper.findComponent(FStartupLoading).exists()).toBe(true);
    expect(wrapper.findComponent(RasterEditor).exists()).toBe(false);
    await advanceFrame();
    await advanceFrame();
    const editor = wrapper.getComponent(RasterEditor);
    expect(editor.props("initialLotSize")).toEqual([90, 203]);
    expect(editor.props("initialSchema")).toBeUndefined();
  } finally {
    wrapper.unmount();
  }
});

it.each([
  { lotSize: [90, 203] as [number, number], size: [90, 203], width: 120, height: 271 },
  { lotSize: null, size: [6, 6], width: 8, height: 8 },
])("opens a saved unbound drawing with size $size", async ({ lotSize, size, width, height }) => {
  const session = useRasterSession();
  session.create(8, 8);
  session.lot.value = { ...session.lot.value, size: [6, 6] };
  vi.mocked(loadAuthoredLot).mockResolvedValue(authoredFromSchema(session.snapshot()));
  const wrapper = mountSection(lotSize);
  try {
    await flushPromises();
    await wrapper.get(".authored-stage:not(.draw-stage)").trigger("click");
    expect(wrapper.getComponent(FSheet).props("open")).toBe(true);
    expect(wrapper.findComponent(RasterEditor).exists()).toBe(false);
    await advanceFrame();
    await advanceFrame();
    const editor = wrapper.getComponent(RasterEditor);
    const schema = editor.props("initialSchema");
    expect(schema?.lot.size).toEqual(size);
    expect(schema?.image).toMatchObject({ width, height });
    editor.vm.$emit("schema", schema);
    await flushPromises();
    expect(saveAuthoredLot).not.toHaveBeenCalled();
  } finally {
    wrapper.unmount();
  }
});

it("opens a replacement drawing without blocking on confirmation and confirms before saving", async () => {
  const session = useRasterSession();
  session.create(8, 8);
  const schema = session.snapshot();
  vi.mocked(loadAuthoredLot).mockResolvedValue(authoredFromSchema(schema));
  const confirm = vi.fn().mockReturnValueOnce(false).mockReturnValueOnce(true);
  vi.stubGlobal("confirm", confirm);
  const wrapper = mountSection([90, 203]);
  try {
    await flushPromises();
    await wrapper.get(".draw-stage").trigger("click");
    expect(confirm).not.toHaveBeenCalled();
    await advanceFrame();
    await advanceFrame();
    const editor = wrapper.getComponent(RasterEditor);
    editor.vm.$emit("schema", schema);
    expect(confirm).toHaveBeenCalledOnce();
    expect(saveAuthoredLot).not.toHaveBeenCalled();
    expect(wrapper.getComponent(FSheet).props("open")).toBe(true);
    editor.vm.$emit("schema", schema);
    await flushPromises();
    expect(saveAuthoredLot).toHaveBeenCalledOnce();
    expect(wrapper.getComponent(FSheet).props("open")).toBe(false);
  } finally {
    wrapper.unmount();
  }
});

it("cancels pending canvas preparation when the sheet is closed", async () => {
  vi.mocked(loadAuthoredLot).mockResolvedValue(null);
  const wrapper = mountSection([90, 203]);
  try {
    await flushPromises();
    await wrapper.get(".draw-stage").trigger("click");
    await advanceFrame();
    await wrapper.get(".sheet-close").trigger("click");
    expect(wrapper.getComponent(FSheet).props("open")).toBe(false);
    expect(frames.size).toBe(0);
    expect(wrapper.findComponent(RasterEditor).exists()).toBe(false);
  } finally {
    wrapper.unmount();
  }
});
