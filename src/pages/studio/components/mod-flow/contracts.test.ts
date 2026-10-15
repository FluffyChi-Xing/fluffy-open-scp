import { describe, expect, it } from "vitest";
import { isEngineManifest } from "./contracts";

describe("engine manifest routing", () => {
  const manifest = {
    format: "openscp.mod",
    manifest_version: 2,
    origin: "openscp",
    engine: { enabled: true, version: 1 },
  };
  it("requires an explicit supported engine contract", () => {
    expect(isEngineManifest(manifest)).toBe(true);
    expect(isEngineManifest({ ...manifest, origin: "community" })).toBe(false);
    expect(
      isEngineManifest({ ...manifest, engine: { enabled: true, version: 2 } }),
    ).toBe(false);
    expect(
      isEngineManifest({ ...manifest, engine: { enabled: false, version: 1 } }),
    ).toBe(false);
  });
  it("keeps old generated inventories and arbitrary package manifests in preview mode", () => {
    expect(isEngineManifest({ generator: "openscp", name: "legacy" })).toBe(
      false,
    );
    expect(isEngineManifest({ name: "community", version: "1.0.0" })).toBe(
      false,
    );
    expect(isEngineManifest({})).toBe(false);
  });
});
