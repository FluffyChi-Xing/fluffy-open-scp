/** @summary Verify URL containment and reject active local styles before browser insertion. */
import { describe, expect, it, vi } from "vitest";
const backend = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => backend);
import { activeAutogenStyles, isInstalledPreviewImage, localImage, localResourcePath, localStylesheet, prepareLocalAssets, resolveLocalImages } from "./local-assets";

describe("local preview boundary", () => {
  it("retains installed-image provenance after switching attachments", async () => {
    backend.invoke.mockResolvedValue({ mime: "image/png", base64: "Ag==" });
    const image = await localImage("installed-game:2F7D0004:00000000:87654320", "provenance-old");
    expect(isInstalledPreviewImage(image)).toBe(true);
    backend.invoke.mockResolvedValue({ ready: true, generation: "provenance-new", assetCount: 1 });
    await prepareLocalAssets("C:/Games/SimCity/SimCityData");
    expect(isInstalledPreviewImage(image)).toBe(true);
    expect(isInstalledPreviewImage("data:image/png;base64,BA==")).toBe(false);
  });
  it("prepares directly from an installation with no extraction or legacy-cache argument", async () => {
    backend.invoke.mockResolvedValue({ ready: true, generation: "direct-1", assetCount: 1 });
    await prepareLocalAssets("C:/Games/SimCity/SimCityData");
    expect(backend.invoke).toHaveBeenLastCalledWith("local_game_assets_prepare", { request: { gameDataPath: "C:/Games/SimCity/SimCityData" } });
  });
  it("retains active nine-slice classes and excludes unused fragment requests", () => {
    expect(activeAutogenStyles('.Used.pinTop{background:url(used-t.png)} .Unused.pinTop{background:url(unused-t.png)}', new Set(["Used"]))).not.toContain("unused-t");
  });
  it("binds typed resource reads to the requested immutable generation", async () => {
    backend.invoke.mockResolvedValue({ mime: "image/png", base64: "AA==" });
    expect(await localImage("installed-game:2F7D0004:00000000:12345678", "direct-2")).toBe("data:image/png;base64,AA==");
    expect(backend.invoke).toHaveBeenLastCalledWith("local_game_assets_resource", { reference: "installed-game:2F7D0004:00000000:12345678", generation: "direct-2" });
  });
  it("retains native layout and property provenance while resolving only actual image payloads", async () => {
    backend.invoke.mockClear();
    backend.invoke.mockResolvedValue({ mime: "image/png", base64: "AQ==" });
    const image = "installed-game:2F7D0004:00000000:87654321";
    const layoutSource = "installed-game:67771F5C:00000000:12345678";
    const propertySource = "installed-game:00B1B104:00000000:ABCDEF01";
    const fixture = { layout: { sourceResource: layoutSource, children: [{ sourceResource: propertySource, drawable: { images: [image] } }] }, assets: { "icon.png": image } };
    const resolved = await resolveLocalImages(fixture, "native-provenance");
    expect(resolved.layout.sourceResource).toBe(layoutSource);
    expect(resolved.layout.children[0]?.sourceResource).toBe(propertySource);
    expect(resolved.layout.children[0]?.drawable.images[0]).toBe("data:image/png;base64,AQ==");
    expect(resolved.assets["icon.png"]).toBe("data:image/png;base64,AQ==");
    expect(backend.invoke.mock.calls).toEqual([["local_game_assets_resource", { reference: image, generation: "native-provenance" }]]);
  });
  it.each(["https://evil.test/a.png", "//evil.test/a.png", "file:///secret", "../../secret", "C:\\secret", "/etc/passwd"])("rejects %s", (path) => {
    expect(() => localResourcePath(path)).toThrow();
  });
  it("resolves relative stylesheet references only within the cache", () => {
    expect(localResourcePath("../assets/x.png", "replica/css/hud.css")).toBe("replica/assets/x.png");
    expect(localResourcePath("/game-ui/replica/assets/x.png")).toBe("replica/assets/x.png");
  });
  it("rejects remote and active CSS before requesting media", async () => {
    backend.invoke.mockResolvedValue({ mime: "text/css", base64: btoa('@import "https://evil.test/style";') });
    await expect(localStylesheet("replica/css/hud.css", "g1")).rejects.toThrow("forbidden");
    backend.invoke.mockResolvedValue({ mime: "text/css", base64: btoa('a {background:url(https://evil.test/a.png)}') });
    await expect(localStylesheet("replica/css/hud.css", "g1")).rejects.toThrow("Forbidden");
  });
  it.each([String.raw`@\69mport "https://evil.test/style";`, String.raw`a{background:u\72l(https://evil.test/a)}`, '@im/**/port "https://evil.test/a";', 'a{background:url("https://evil.test/a b")}', 'a{background:image-set("https://evil.test/a" 1x)}', 'a{background:-webkit-image-set("https://evil.test/a" 1x)}'])("rejects obfuscated network CSS %s", async (css) => {
    backend.invoke.mockResolvedValue({ mime: "text/css", base64: btoa(css) });
    await expect(localStylesheet("replica/css/hud.css", "g1")).rejects.toThrow();
  });
  it("discards only the original invalid border declarations without enabling CSS escapes", async () => {
    backend.invoke.mockResolvedValue({ mime: "text/css", base64: btoa(String.raw`a {
      \\border: solid 1px rgb(255,255,255);
      color: white;
    }`) });
    expect(await localStylesheet("replica/css/hud.css", "g1")).not.toContain("\\");
  });
});
