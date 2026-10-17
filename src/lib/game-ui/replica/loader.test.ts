/** @summary Verify local language isolation, generation binding and safe failure recovery. */
import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => backend);

/** @summary Emulate command responses without importing executable game content. */
function installBackend(): void {
  backend.invoke.mockImplementation(async (command: string, args?: { locale?: string; generation?: string }) => {
    if (command === "local_game_assets_status") return { ready: true, generation: "g1", assetCount: 6 };
    if (command !== "local_game_assets_model") throw new Error("Unexpected legacy resource-cache command");
    return { previewGeneration: args?.generation, layout: { width: 1600, height: 900 }, assets: {}, palette: { byCategory: {}, row: {} }, model: { categories: [{ id: "ROAD", label: args?.locale === "pl-PL" ? "Drogi" : args?.locale === "zh-CN" ? "Original roads" : "Roads" }], row: {} }, tools: { ROAD: [] }, locale: { table: { id: "City" } } };
  });
}

describe("local replica data", () => {
  beforeEach(() => { vi.resetModules(); backend.invoke.mockReset(); installBackend(); });

  it("preserves the Traditional Chinese locale identity used by the original authoring data", async () => {
    const { gameTextLocale, installedGameLanguage, loadReplicaData } = await import("./loader");
    expect(gameTextLocale("zh-TW")).toBe("zh-TW");
    expect(gameTextLocale("zh-Hant")).toBe("zh-TW");
    expect(installedGameLanguage("zh-TW", ["zh-TW", "zh-CN"])).toBe("zh-TW");
    await loadReplicaData("zh-TW");
    expect(backend.invoke).toHaveBeenCalledWith("local_game_assets_model", { locale: "zh-TW", generation: "g1" });
  });

  it("preserves the original authoring locale by default and keeps language requests isolated", async () => {
    const { loadReplicaData } = await import("./loader");
    expect((await loadReplicaData()).model.categories[0]?.label).toBe("Roads");
    expect(backend.invoke).toHaveBeenCalledWith("local_game_assets_model", { locale: "zh-TW", generation: "g1" });
    expect((await loadReplicaData("pl-PL")).model.categories[0]?.label).toBe("Drogi");
    expect((await loadReplicaData("zh-CN")).model.categories[0]?.label).toBe("Original roads");
    expect(document.head.querySelector('script[src*="game-ui"]')).toBeNull();
    const first = await loadReplicaData();
    expect(await loadReplicaData()).toBe(first);
  });

  it("rejects an unconfigured provider", async () => {
    backend.invoke.mockResolvedValue({ ready: false });
    const { loadReplicaData } = await import("./loader");
    await expect(loadReplicaData()).rejects.toThrow("not configured");
  });

  it("rejects model data bound to a different installation generation", async () => {
    backend.invoke.mockImplementation(async (command: string) => command === "local_game_assets_status" ? { ready: true, generation: "g1", assetCount: 1 } : { previewGeneration: "g2" });
    const { loadReplicaData } = await import("./loader");
    await expect(loadReplicaData()).rejects.toThrow("generation changed");
  });

  it("chooses the available installed language without inventing an absent locale", async () => {
    const { installedGameLanguage } = await import("./loader");
    expect(installedGameLanguage("en-US", ["pl-PL"])).toBe("pl-PL");
    expect(installedGameLanguage("pl-PL", ["en-US", "pl-PL"])).toBe("pl-PL");
    expect(() => installedGameLanguage("en-US", [])).toThrow("No supported");
  });

  it("preserves native coverage and diagnostic metadata as inert data", async () => {
    const implementation = backend.invoke.getMockImplementation()!;
    backend.invoke.mockImplementation(async (command: string, args?: object) => {
      const value = await implementation(command, args);
      return command === "local_game_assets_model" ? { ...value, diagnostics: ["<img src=x onerror=alert(1)>"], requiredStyleClasses: ["NineSlice"], originalDrawableTypeCounts: { "1": 3 } } : value;
    });
    const { loadReplicaData } = await import("./loader");
    expect(await loadReplicaData()).toMatchObject({ diagnostics: ["<img src=x onerror=alert(1)>"], requiredStyleClasses: ["NineSlice"], originalDrawableTypeCounts: { "1": 3 } });
    expect(document.querySelector("img[src=x]")).toBeNull();
  });

  it("rejects an installation changed between the initial status and completed model read", async () => {
    const implementation = backend.invoke.getMockImplementation()!;
    let statusReads = 0;
    backend.invoke.mockImplementation(async (command: string, args?: object) => command === "local_game_assets_status" ? { ready: true, generation: ++statusReads === 1 ? "g1" : "g2", assetCount: 1 } : implementation(command, args));
    const { loadReplicaData } = await import("./loader");
    await expect(loadReplicaData()).rejects.toThrow("generation changed");
  });

  it("rejects baseline category icons that are not typed installed resources", async () => {
    const implementation = backend.invoke.getMockImplementation()!;
    backend.invoke.mockImplementation(async (command: string, args?: object) => {
      const value = await implementation(command, args);
      if (command === "local_game_assets_model") value.model.categories[0].icon = "https://invalid.test/icon.png";
      return value;
    });
    const { loadReplicaData } = await import("./loader");
    await expect(loadReplicaData()).rejects.toThrow("category icon");
  });

  it("retries failures and passes the immutable generation on every data read", async () => {
    const { loadReplicaData } = await import("./loader");
    const implementation = backend.invoke.getMockImplementation()!;
    backend.invoke.mockImplementationOnce(implementation).mockRejectedValueOnce(new Error("cache changed"));
    await expect(loadReplicaData()).rejects.toThrow("cache changed");
    expect((await loadReplicaData()).model.categories[0]?.label).toBe("Roads");
    for (const [command, args] of backend.invoke.mock.calls) {
      if (command === "local_game_assets_model") expect(args).toMatchObject({ generation: "g1" });
    }
  });
});

