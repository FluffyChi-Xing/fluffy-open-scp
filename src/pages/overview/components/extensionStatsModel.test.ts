import { describe, expect, it } from "vitest";
import type { PackageStatistics } from "@/api/tauri";
import {
  colorOf,
  compressionRatio,
  countRankBars,
  formatCompact,
  knownCoveragePercent,
  mergeExtensions,
  sizeRankBars,
  sparklinePath,
} from "./extensionStatsModel";

const sample = (): PackageStatistics => ({
  packages: [
    {
      path: "/a.package",
      name: "a",
      fileSize: 50,
      entryCount: 10,
      knownDecompressed: 80,
      unknownDecompressed: 20,
      extensions: [],
    },
    {
      path: "/b.package",
      name: "b",
      fileSize: 30,
      entryCount: 30,
      knownDecompressed: 40,
      unknownDecompressed: 0,
      extensions: [],
    },
  ],
  failed: [],
  extensions: [
    {
      ext: "rw4",
      typeId: 1,
      known: true,
      count: 10,
      storedSize: 20,
      decompressedSize: 80,
    },
    {
      ext: "property",
      typeId: 2,
      known: true,
      count: 90,
      storedSize: 10,
      decompressedSize: 20,
    },
  ],
  totals: {
    fileSize: 80,
    storedSize: 30,
    decompressedSize: 100,
    knownDecompressed: 100,
    unknownDecompressed: 0,
    knownCount: 100,
    unknownCount: 0,
    entryCount: 100,
  },
});

describe("extensionStatsModel", () => {
  it("keeps count ranking orthogonal to size ranking", () => {
    const slices = mergeExtensions(sample().extensions);
    expect(countRankBars(slices, 2).map((bar) => bar.label)).toEqual([
      "property",
      "rw4",
    ]);
    expect(sizeRankBars(slices, 2).map((bar) => bar.label)).toEqual([
      "rw4",
      "property",
    ]);
  });

  it("assigns a stable palm color per extension name", () => {
    expect(colorOf({ ext: "rw4", known: true })).toBe(
      colorOf({ ext: "rw4", known: true }),
    );
    expect(colorOf({ ext: "rw4", known: true })).not.toBe(
      colorOf({ ext: "property", known: true }),
    );
  });

  it("reports coverage and compression from totals", () => {
    const stats = sample();
    expect(knownCoveragePercent(stats)).toBe(100);
    expect(compressionRatio(stats)).toBeCloseTo(0.3);
    expect(formatCompact(7380)).toBe("7.38k");
  });

  it("builds a sparkline path for ranked package points", () => {
    const path = sparklinePath([
      { label: "a", value: 10 },
      { label: "b", value: 30 },
    ]);
    expect(path.startsWith("M ")).toBe(true);
    expect(path.includes(" L ")).toBe(true);
  });
});
