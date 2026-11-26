import { createPinia } from "pinia";
import { flushPromises, shallowMount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import { i18n } from "@/locales";
import ProjectRepoCodeTab from "./ProjectRepoCodeTab.vue";

vi.mock("@/api", () => ({
  isTauri: () => true,
  tauriApi: {
    studio: { projects: { repoSummary: vi.fn() } },
    workspace: {
      codeReadText: vi.fn().mockRejectedValue(new Error("missing README")),
      codeOpenInExplorer: vi.fn().mockResolvedValue(undefined),
    },
  },
}));

it("refreshes releases and reveals the asset and its optional Panel runtime independently", async () => {
  const empty = {
    author: "",
    version: "",
    modType: "road",
    description: "",
    releases: [],
    languages: [],
    languageTruncated: false,
    totalSourceBytes: 0,
  };
  const relativePath = ".build/abc123/road.package";
  const assetPath = ".build/def456/packages/building.package";
  const runtimeName = "openscp-panel-runtime-v1.package";
  const runtimePath = `.build/def456/dependencies/${runtimeName}`;
  vi.mocked(tauriApi.studio.projects.repoSummary)
    .mockResolvedValueOnce(empty)
    .mockResolvedValueOnce({
      ...empty,
      releases: [
        { name: "road.package", relativePath, buildHash: "abc123", size: 42, modifiedMs: 0 },
        { name: "building.package", relativePath: assetPath, buildHash: "def456", size: 100,
          modifiedMs: 0, panelRuntime: { name: runtimeName, relativePath: runtimePath, size: 42 } },
      ],
    });
  const locale = i18n.global.locale.value;
  i18n.global.locale.value = "en-US";
  const wrapper = shallowMount(ProjectRepoCodeTab, {
    props: {
      project: { id: 1, name: "Road", relPath: "road-test", status: "active", folderExists: true, createdAt: 0, updatedAt: 0 },
      groupName: "",
    },
    global: { plugins: [createPinia(), i18n] },
  });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain("No packages have been built yet");
    wrapper.getComponent({ name: "ProjectRepoFileTree" }).vm.$emit("refresh");
    await flushPromises();
    const release = wrapper.get("button.release-package");
    expect(release.text()).toBe("road.package");
    expect(release.attributes("title")).toBe("Show road.package in the file manager");
    await release.trigger("click");
    expect(tauriApi.workspace.codeOpenInExplorer).toHaveBeenCalledWith("road-test", relativePath);
    const rows = wrapper.findAll(".release-list li");
    expect(rows[0]!.findAll("button.release-package")).toHaveLength(1);
    const [asset, runtime] = rows[1]!.findAll("button.release-package");
    expect(asset!.text()).toBe("building.package");
    expect(runtime!.text()).toBe(runtimeName);
    expect(rows[1]!.text()).toContain("Uni Panel from an older build");
    await asset!.trigger("click");
    expect(tauriApi.workspace.codeOpenInExplorer).toHaveBeenLastCalledWith("road-test", assetPath);
    await runtime!.trigger("click");
    expect(tauriApi.workspace.codeOpenInExplorer).toHaveBeenLastCalledWith("road-test", runtimePath);
    i18n.global.locale.value = "zh-CN";
    await wrapper.vm.$nextTick();
    expect(release.attributes("title")).toBe("在文件管理器中定位 road.package");
    expect(rows[1]!.text()).toContain("旧构建附带的 Uni Panel");
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = locale;
  }
});
