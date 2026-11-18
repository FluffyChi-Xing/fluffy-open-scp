import { mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import type { ModProjectView } from "@/api/tauri";
import { i18n } from "@/locales";
import { useModProjectsStore } from "@/stores/modProjects";
import StudioPage from "./index.vue";

vi.mock("@/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/api")>()),
  isTauri: () => true,
}));

it("paginates the filtered projects and updates pagination text in place", async () => {
  const pinia = createPinia();
  const store = useModProjectsStore(pinia);
  vi.spyOn(store, "loadAll").mockResolvedValue();
  store.devRoot = "mods";
  store.projects = Array.from({ length: 23 }, (_, index): ModProjectView => ({
    id: index + 1,
    name: `Project ${index + 1}`,
    relPath: `project-${index + 1}`,
    groupId: index < 11 ? 1 : undefined,
    status: "active",
    createdAt: 0,
    updatedAt: 0,
    folderExists: true,
  }));

  const previousLocale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const page = mount(StudioPage, {
    global: {
      plugins: [pinia, i18n],
      stubs: {
        RouterLink: true,
        ProjectDashboard: true,
        BuildActionsCard: true,
        ProjectFormSheet: true,
        GroupManagerSheet: true,
      },
    },
  });

  const nav = page.get(".project-pagination");
  const [previous, next] = nav.findAll("button");
  const projectNames = () => page.findAll(".name-cell strong").map((cell) => cell.text());
  try {
    expect(projectNames()).toHaveLength(10);
    expect(previous.attributes("disabled")).toBeDefined();
    await next.trigger("click");
    await next.trigger("click");
    expect(projectNames()).toEqual(["Project 21", "Project 22", "Project 23"]);
    expect(next.attributes("disabled")).toBeDefined();
    expect(nav.text()).toContain("显示第 21–23 项，共 23 项");

    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(nav.attributes("aria-label")).toBe("Project list pagination");
    expect(nav.text()).toContain("Page 3 of 3 · Showing 21–23 of 23 projects");
    expect(previous.text()).toBe("Previous");

    store.projects = store.projects.slice(0, 22);
    await page.findAll(".group-tab")[1].trigger("click");
    expect(projectNames()[0]).toBe("Project 12");
    expect(nav.text()).toContain("Page 1 of 2");
    await next.trigger("click");
    expect(projectNames()).toEqual(["Project 22"]);

    store.projects = store.projects.slice(0, 21);
    await nextTick();
    expect(projectNames()).toHaveLength(10);
    expect(nav.text()).toContain("Page 1 of 1");
  } finally {
    page.unmount();
    i18n.global.locale.value = previousLocale;
  }
});
