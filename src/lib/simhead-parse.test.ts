import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseLotModelContainer } from "@/lib/three-gltf";

// 取证样本在 tmp/（gitignore，清理豁免但非必需）；缺席时跳过。
const sample = "tmp/simhead.lotm";

describe("sim head lotm", () => {
  it.skipIf(!existsSync(sample))("normalPng is the shared atlas (not tex#0)", () => {
    const buffer = readFileSync(sample);
    const payload = parseLotModelContainer(
      buffer.buffer.slice(
        buffer.byteOffset,
        buffer.byteOffset + buffer.byteLength,
      ),
    );
    const material = payload.materials[0];
    expect(material.normalPng?.length).toBe(35027);
    expect(material.slot0Png?.length).toBe(220);
  });
});
