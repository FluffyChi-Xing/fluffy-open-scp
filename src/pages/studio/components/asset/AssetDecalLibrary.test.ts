import { flushPromises, mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { command } from "@/api/tauri";
import { i18n } from "@/locales";
import AssetDecalLibrary from "./AssetDecalLibrary.vue";

vi.mock("@/api/tauri", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/api/tauri")>(),
  command: vi.fn(),
}));

it("opens only the selected package and hands its atlas to the shared preview", async () => {
  const tgi = { typeId: 0x00b1b104, group: 0xfb661651, instance: 0xaa8b7058 };
  vi.mocked(command).mockResolvedValueOnce([
    { packageId: 1, path: "D:\\Game\\SimCity_Game.package", atlases: [{ tgi, entryCount: 254 }] },
    { packageId: 2, path: "D:\\Game\\SimCity_Graphics.package", atlases: [{ tgi, entryCount: 112 }] },
  ]);
  const wrapper = mount(AssetDecalLibrary, {
    props: { editEnabled: true },
    global: {
      plugins: [i18n, createPinia()],
      stubs: {
        DecalDictionaryGallery: {
          name: "DecalDictionaryGallery",
          props: { packageId: Number, tgi: Object, compact: Boolean, placeable: Boolean },
          template: "<div />",
        },
      },
    },
  });
  try {
    await flushPromises();
    const groups = wrapper.findAll("details");
    expect(groups.map((group) => group.get("summary").text())).toEqual([
      "SimCity_Game.package 254", "SimCity_Graphics.package 112",
    ]);
    expect(wrapper.findComponent({ name: "DecalDictionaryGallery" }).exists()).toBe(false);
    (groups[1].element as HTMLDetailsElement).open = true;
    await groups[1].trigger("toggle");
    const preview = wrapper.getComponent({ name: "DecalDictionaryGallery" });
    expect(preview.props()).toMatchObject({
      packageId: 2, tgi, compact: true, placeable: true,
    });
    const id = { typeId: 0, group: 0, instance: 0x17b244b8 };
    preview.vm.$emit("place", { id });
    expect(wrapper.emitted("place")).toEqual([[{
      id, source: { packagePath: "D:\\Game\\SimCity_Graphics.package", atlas: tgi },
    }]]);
    (groups[1].element as HTMLDetailsElement).open = false;
    await groups[1].trigger("toggle");
    expect(wrapper.findComponent({ name: "DecalDictionaryGallery" }).exists()).toBe(false);
  } finally {
    wrapper.unmount();
  }
});

it("updates the empty catalog message when the locale changes", async () => {
  vi.mocked(command).mockResolvedValueOnce([]);
  const previousLocale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(AssetDecalLibrary, { global: { plugins: [i18n, createPinia()] } });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain("已打开的包中没有可预览的贴花。");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("The open packages contain no decals to preview.");
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = previousLocale;
  }
});
