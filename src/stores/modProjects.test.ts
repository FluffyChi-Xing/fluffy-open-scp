import { createPinia, setActivePinia } from "pinia";
import { expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import { useModProjectsStore } from "./modProjects";
vi.mock("@/api", async (original) => ({
  ...(await original<typeof import("@/api")>()),
  isTauri: () => true,
}));
it("initializes the managed mod directory on a fresh dev origin while loading the project list", async () => {
  localStorage.setItem("openscp:mod-dev-root", "D:/rust/packages/fluffy-open-scp/target/debug/mods");
  setActivePinia(createPinia());
  const status = { path: "D:/openscp/mods", exists: true, isDirectory: true };
  vi.spyOn(tauriApi.studio, "setupStatus").mockResolvedValue({
    workspace: status,
    game: status,
    modRoot: status,
    onboardingCompleted: true,
  });
  vi.spyOn(tauriApi.studio, "legacyStatus").mockRejectedValue(
    new Error("optional status unavailable"),
  );
  vi.spyOn(tauriApi.studio.projects, "list").mockResolvedValue([]);
  vi.spyOn(tauriApi.studio.groups, "list").mockResolvedValue([]);
  vi.spyOn(tauriApi.studio, "stats").mockResolvedValue(
    {} as Awaited<ReturnType<typeof tauriApi.studio.stats>>,
  );
  const store = useModProjectsStore();
  expect(store.devRoot).toBe("");
  await store.loadAll();
  expect(store.devRoot).toBe("D:/openscp/mods");
  expect(localStorage.getItem("openscp:mod-dev-root")).toBe("D:/openscp/mods");
  vi.restoreAllMocks();
});
