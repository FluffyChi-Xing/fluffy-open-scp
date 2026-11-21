import { defineComponent } from "vue";
import { mount, flushPromises } from "@vue/test-utils";
import { afterEach, expect, it, vi } from "vitest";
import {
  useEditorPackages,
  type EditorPackageScope,
} from "./useEditorPackages";
const mock = vi.hoisted(() => ({ command: vi.fn(), settings: vi.fn() }));
vi.mock("@/api", () => ({
  isTauri: () => true,
  tauriApi: { settings: { get: mock.settings } },
}));
vi.mock("@/api/tauri", () => ({ command: mock.command }));
afterEach(() => vi.resetAllMocks());
function editor(kind: "assets" | "terrain" = "assets") {
  let scope!: EditorPackageScope;
  const wrapper = mount(
    defineComponent({
      setup() {
        scope = useEditorPackages(kind);
        return () => null;
      },
    }),
  );
  return {
    wrapper,
    get scope() {
      return scope;
    },
  };
}
it("loads the configured directory independently and releases only its owned handles", async () => {
  mock.settings.mockResolvedValue({ gameDataPath: "Game/SimCityData" });
  mock.command.mockImplementation(async (name) =>
    name === "acquire_editor_packages"
      ? {
          packages: [
            { packageId: 101, path: "Game/SimCityData/SimCity_Game.package" },
          ],
          missing: ["SimCityDataEP1.package"],
        }
      : undefined,
  );
  const page = editor();
  await flushPromises();
  expect(mock.command).toHaveBeenCalledWith("acquire_editor_packages", {
    request: { directory: "Game/SimCityData", kind: "assets" },
  });
  expect(page.scope.opened[0].package.packageId).toBe(101);
  expect(page.scope.missing).toEqual(["SimCityDataEP1.package"]);
  expect(page.scope.failed).toBe(false);
  page.wrapper.unmount();
  expect(mock.command).toHaveBeenCalledWith("release_editor_packages", {
    packageIds: [101],
  });
  expect(
    mock.command.mock.calls.some(([name]) => name === "close_package"),
  ).toBe(false);
});
it("releases late acquisitions after navigating away", async () => {
  mock.settings.mockResolvedValue({ gameDataPath: "Game" });
  let finish!: (result: unknown) => void;
  mock.command.mockImplementation((name) =>
    name === "acquire_editor_packages"
      ? new Promise((resolve) => {
          finish = resolve;
        })
      : Promise.resolve(),
  );
  const page = editor("terrain");
  await flushPromises();
  page.wrapper.unmount();
  finish({
    packages: [{ packageId: 202, path: "Game/Terrain.package" }],
    missing: [],
  });
  await flushPromises();
  expect(mock.command).toHaveBeenCalledWith("release_editor_packages", {
    packageIds: [202],
  });
  expect(page.scope.opened).toEqual([]);
});
it("reloads a manually selected directory and keeps partial results usable", async () => {
  mock.settings.mockResolvedValue({ gameDataPath: "Old" });
  let id = 10;
  mock.command.mockImplementation(async (name) =>
    name === "acquire_editor_packages"
      ? {
          packages: [{ packageId: ++id, path: "Terrain.package" }],
          missing: ["other.package"],
        }
      : undefined,
  );
  const page = editor("terrain");
  await flushPromises();
  await page.scope.load("Chosen");
  expect(page.scope.directory).toBe("Chosen");
  expect(page.scope.opened[0].package.packageId).toBe(12);
  expect(mock.command).toHaveBeenCalledWith("release_editor_packages", {
    packageIds: [11],
  });
  page.wrapper.unmount();
});
it("owns manually opened asset sources and shares repeated reads until unmount", async () => {
  mock.settings.mockResolvedValue({ gameDataPath: "" });
  mock.command.mockImplementation(async (name) =>
    name === "acquire_editor_file"
      ? { packageId: 303, path: "Asset/reference.package" }
      : undefined,
  );
  const page = editor();
  await flushPromises();
  const [a, b] = await Promise.all([
    page.scope.openFile("Asset/reference.package"),
    page.scope.openFile("Asset/reference.package"),
  ]);
  expect(a.package.packageId).toBe(303);
  expect(a).toBe(b);
  expect(
    mock.command.mock.calls.filter(([name]) => name === "acquire_editor_file"),
  ).toHaveLength(1);
  page.wrapper.unmount();
  expect(mock.command).toHaveBeenCalledWith("release_editor_packages", {
    packageIds: [303],
  });
});
