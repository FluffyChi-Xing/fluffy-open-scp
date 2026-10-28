import { describe, expect, it } from "vitest";
import type { PathSourceSummary } from "@/api/tauri";
import {
  entryDisplayLabel,
  filterPathSources,
  paginate,
  selectPathSourcePage,
  sourceDisplayLabel,
} from "./sources";

const row = (
  instance: number,
  name: string,
  pathCount = 2,
): PathSourceSummary => ({
  packageId: 1,
  typeId: 0x00b1b104,
  group: 0x42e1c000,
  instance,
  name,
  pathCount,
  pointCount: pathCount * 4,
  entryIds: [0x418a18b1],
});

const rows = [
  row(0xdbf54200, "C_Gen_W1D2_Store_02_01_N"),
  row(0x1a7490a1, ""),
  row(0x86e9ebc7, "Dumpster Row"),
];

describe("filterPathSources", () => {
  it("matches registry names case-insensitively", () => {
    expect(filterPathSources(rows, "dumpster")).toHaveLength(1);
  });
  it("matches instance hex with or without 0x prefix", () => {
    expect(filterPathSources(rows, "1a7490a1")).toHaveLength(1);
    expect(filterPathSources(rows, "0x1A7490A1")).toHaveLength(1);
  });
  it("returns everything for empty keyword", () => {
    expect(filterPathSources(rows, "")).toHaveLength(3);
  });
});

describe("paginate", () => {
  it("slices and reports page count", () => {
    const page = paginate(rows, 1, 2);
    expect(page.items).toHaveLength(2);
    expect(page.pageCount).toBe(2);
    expect(page.total).toBe(3);
  });
  it("clamps out-of-range pages to the last page", () => {
    expect(paginate(rows, 99, 2).page).toBe(2);
    expect(paginate(rows, 0, 2).page).toBe(1);
  });
  it("selectPathSourcePage composes filter and pagination", () => {
    const page = selectPathSourcePage(rows, "store", 1, 5);
    expect(page.total).toBe(1);
    expect(page.items[0].name).toContain("Store");
  });
});

describe("labels", () => {
  it("prefers registry names and falls back to hex", () => {
    expect(sourceDisplayLabel({ instance: 0x12, label: "Willis" })).toBe(
      "Willis",
    );
    expect(sourceDisplayLabel({ instance: 0x12 })).toBe("0x12");
    expect(entryDisplayLabel("418a18b1")).toBe("0x418A18B1");
    expect(entryDisplayLabel(null)).toBe("");
  });
});
