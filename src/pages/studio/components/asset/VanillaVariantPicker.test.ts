import { flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import { i18n } from "@/locales";
import VanillaVariantPicker from "./VanillaVariantPicker.vue";
import VanillaVariantMenuBrowser from "./VanillaVariantMenuBrowser.vue";

it("uses the scanned fire station binding before the user confirms it", async () => {
  const read = vi.spyOn(tauriApi.workspace, "codeReadText").mockRejectedValue(new Error("project not initialized"));
  const settings = vi.spyOn(tauriApi.settings, "get").mockResolvedValue({ gameDataPath: "Game/SimCityData" });
  const lookup = vi.spyOn(tauriApi.workspace, "vanillaVariantLookup").mockResolvedValue([]);
  const select = vi.fn().mockResolvedValue(true);
  const wrapper = mount(VanillaVariantPicker, {
    props: { open: true, inline: true, project: "variant", schemaPath: "asset.schema.json", select },
    global: { plugins: [i18n], stubs: { VanillaVariantMenuBrowser: true } },
  });
  try {
    await flushPromises();
    wrapper.getComponent(VanillaVariantMenuBrowser).vm.$emit("lookup", "6D0235EE", "Fire Station");
    await nextTick();
    expect(wrapper.get(".choices").text()).toContain("Fire Station");
    expect(wrapper.get('input[type="radio"]').element).toHaveProperty("checked", true);
    expect(lookup).not.toHaveBeenCalled();
    expect(select).not.toHaveBeenCalled();
    await wrapper.get(".choices button").trigger("click");
    expect(select).toHaveBeenCalledWith({
      gameData: "Game/SimCityData", scripts: "",
      unit: "00B1B104:40E0C000:E917279C", lot: "00B1B104:40E1C000:E917279C",
    });
  } finally {
    wrapper.unmount();
    read.mockRestore();
    settings.mockRestore();
    lookup.mockRestore();
  }
});

it("switches the active picker language and hands the selected original to the scene workbench", async () => {
  const previousLocale = i18n.global.locale.value;
  const source = { gameData: "Game/SimCityData", scripts: "Game/Scripts.package" };
  const match = { name: "Test factory", unit: "00B1B104:40E0C000:14FF6F1D", lot: "00B1B104:40B9A133:14FF6F1D", matched: true, primary: false };
  const tgi = "00B1B104:43E1C000:39B64608";
  const content = JSON.stringify({ vanillaVariant: source });
  const read = vi.spyOn(tauriApi.workspace, "codeReadText").mockResolvedValue({
    content, revision: "test", relativePath: "package.json", size: content.length,
  });
  const search = vi.spyOn(tauriApi.workspace, "vanillaVariantLookup").mockResolvedValue([match]);
  const settings = vi.spyOn(tauriApi.settings, "get").mockResolvedValue({});
  const select = vi.fn().mockResolvedValue(true);
  const wrapper = mount(VanillaVariantPicker, {
    props: { open: false, project: "variant", schemaPath: "asset.schema.json", select },
    global: { plugins: [i18n], stubs: { VanillaVariantMenuBrowser: true, FSheet: { template: "<section><slot /></section>" } } },
  });
  try {
    await wrapper.setProps({ open: true });
    await flushPromises();
    i18n.global.locale.value = "zh-CN";
    await nextTick();
    await wrapper.findAll(".mode-switch button")[1].trigger("click");
    expect(wrapper.get('button[type="submit"]').text()).toBe("解析建筑本体");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.get('button[type="submit"]').text()).toBe("Resolve building");
    await wrapper.get('input[maxlength="40"]').setValue(tgi);
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(search).toHaveBeenCalledWith({ ...source, tgi });
    expect(select).not.toHaveBeenCalled();
    await wrapper.get(".choices button").trigger("click");
    await flushPromises();
    expect(select).toHaveBeenCalledWith({ ...source, unit: match.unit, lot: match.lot });
    expect(wrapper.emitted("update:open")).toEqual([[false]]);
  } finally {
    wrapper.unmount();
    read.mockRestore();
    search.mockRestore();
    settings.mockRestore();
    i18n.global.locale.value = previousLocale;
  }
});
