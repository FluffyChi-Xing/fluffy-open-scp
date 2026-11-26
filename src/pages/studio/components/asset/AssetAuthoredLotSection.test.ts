import { flushPromises, shallowMount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
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
