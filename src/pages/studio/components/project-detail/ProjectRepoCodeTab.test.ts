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

it("refreshes road releases and reveals the exact package from its filename", async () => {
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
  vi.mocked(tauriApi.studio.projects.repoSummary)
    .mockResolvedValueOnce(empty)
    .mockResolvedValueOnce({
      ...empty,
      releases: [{ name: "road.package", relativePath, buildHash: "abc123", size: 42, modifiedMs: 0 }],
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
    i18n.global.locale.value = "zh-CN";
    await wrapper.vm.$nextTick();
    expect(release.attributes("title")).toBe("在文件管理器中定位 road.package");
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = locale;
  }
});
