import { flushPromises, shallowMount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { command } from "@/api/tauri";
import { i18n } from "@/locales";
import RoadAssetWorkbench from "./RoadAssetWorkbench.vue";

vi.mock("@/api/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/api/tauri")>()),
  command: vi.fn(),
}));

it("requires a paved base before offering strips and keeps the cross-section empty on selection", async () => {
  const schema = {
    format: "openscp.road-asset/1",
    recipe: "highStreet",
    identity: 42,
    title: "Road",
    description: "",
    gamePackage: "SimCity_Game.package",
    scriptsPackage: "SimCity-Scripts.package",
    section: [],
  };
  const project = {
    schema,
    manifest: { dependencies: { road: {} } },
    manifestRevision: "a",
    schemaRevision: "b",
  };
  const locale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  vi.mocked(command).mockResolvedValue(project);
  const wrapper = shallowMount(RoadAssetWorkbench, {
    props: { project: "roads", manifest: project.manifest },
    global: {
      plugins: [i18n],
      stubs: {
        SceneWorkbenchLayout: {
          template:
            '<slot name="viewport"/><slot name="lower"/><slot name="library"/>',
        },
      },
    },
  });
  try {
    await flushPromises();
    const frontage = wrapper.getComponent({ name: "Switch" });
    expect(frontage.props("modelValue")).toBe(true);
    expect(frontage.props("ariaLabel")).toBe("允许划区和建筑贴路");
    frontage.vm.$emit("update:modelValue", false);
    await wrapper.vm.$nextTick();
    expect(frontage.props("modelValue")).toBe(false);
    i18n.global.locale.value = "en-US";
    await wrapper.vm.$nextTick();
    expect(frontage.props("ariaLabel")).toBe(
      "Allow zoning and roadside building placement",
    );
    expect(wrapper.findComponent({ name: "RoadStripLibrary" }).exists()).toBe(
      false,
    );
    const basePicker = wrapper.getComponent({ name: "FSelect" });
    expect(
      basePicker
        .props("options")
        .map((option: { value: string }) => option.value),
    ).not.toContain("dirt");
    const chooseBase = async (recipe: string) => {
      basePicker.vm.$emit("update:modelValue", recipe);
      await wrapper.vm.$nextTick();
    };
    await chooseBase("lowStreet");
    expect(
      wrapper.getComponent({ name: "RoadDraftSection" }).props("strips"),
    ).toEqual([]);
    const library = wrapper.getComponent({ name: "RoadStripLibrary" });
    expect(Boolean(
      basePicker.element.compareDocumentPosition(frontage.element) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    )).toBe(true);
    expect(Boolean(
      frontage.element.compareDocumentPosition(library.element) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    )).toBe(true);
    expect(library.props("widths").forward).toBe(4.5);
    library.vm.$emit("add", "forward");
    await wrapper.vm.$nextTick();
    expect(wrapper.getComponent({ name: "RoadPreview" }).props()).toMatchObject(
      {
        recipe: "lowStreet",
        draft: [{ kind: "forward", width: 4.5 }],
      },
    );
    await chooseBase("mediumStreet");
    const [, lightingPicker] = wrapper.findAllComponents({ name: "FSelect" });
    expect(lightingPicker.props("options")).toContainEqual({
      value: "both",
      label: "Both sides",
    });
    lightingPicker.vm.$emit("update:modelValue", "right");
    i18n.global.locale.value = "zh-CN";
    await wrapper.vm.$nextTick();
    expect(lightingPicker.props("modelValue")).toBe("right");
    expect(lightingPicker.props("options")).toContainEqual({
      value: "right",
      label: "右侧",
    });
    expect(wrapper.getComponent({ name: "RoadPreview" }).props()).toMatchObject(
      {
        recipe: "mediumStreet",
        draft: [{ kind: "forward", width: 4.5 }],
        streetlights: "right",
      },
    );
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = locale;
    vi.mocked(command).mockReset();
  }
});
