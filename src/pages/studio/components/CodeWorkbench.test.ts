import { createPinia } from "pinia";
import { flushPromises, shallowMount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import { i18n } from "@/locales";
import CodeWorkbench from "./CodeWorkbench.vue";
import ModAssetWorkbench from "./asset/ModAssetWorkbench.vue";

vi.mock("@/api", async (load) => {
  const api = await load<typeof import("@/api")>();
  return {
    ...api,
    isTauri: () => true,
    tauriApi: {
      ...api.tauriApi,
      workspace: { ...api.tauriApi.workspace, codeTree: vi.fn(), codeManifest: vi.fn() },
    },
  };
});
vi.mock("./asset/ModAssetWorkbench.vue", () => ({
  default: { name: "ModAssetWorkbench", props: ["project"] },
}));

it.each([true, false])("routes an asset manifest with engine enabled=%s", async (enabled) => {
  vi.mocked(tauriApi.workspace.codeTree).mockResolvedValue({
    rootPath: "D:/mods/Test asset", fileCount: 1, folderCount: 8, truncated: false, entries: [],
  });
  vi.mocked(tauriApi.workspace.codeManifest).mockResolvedValue({
    existed: true, created: false,
    manifest: {
      format: "openscp.mod", manifest_version: 2, origin: "openscp", mod_type: "assets",
      engine: { enabled, version: 1 },
      ...(enabled ? { workflow: "assets" } : {}),
    },
  });
  const wrapper = shallowMount(CodeWorkbench, {
    props: {
      embedded: true,
      project: {
        id: 1, name: "Test asset", relPath: "Test asset", status: "active",
        folderExists: true, createdAt: 0, updatedAt: 0,
      },
    },
    global: { plugins: [createPinia(), i18n] },
  });
  try {
    await flushPromises();
    expect(wrapper.findComponent(ModAssetWorkbench).exists()).toBe(enabled);
    expect(wrapper.text().includes(i18n.global.t("flow.initialize"))).toBe(!enabled);
    if (enabled) expect(wrapper.getComponent(ModAssetWorkbench).props("project")).toBe("Test asset");
  } finally {
    wrapper.unmount();
  }
});
