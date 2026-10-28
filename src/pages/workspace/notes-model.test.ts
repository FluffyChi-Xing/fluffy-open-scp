import { describe, expect, it } from "vitest";
import type { ResourceAnnotation } from "@/api/tauri";
import {
  countResources,
  filterGroups,
  groupByTopic,
  matchAnnotation,
  tgiInstanceOf,
  tgiTextOf,
  uniformPackageName,
} from "./notes-model";

function note(overrides: Partial<ResourceAnnotation>): ResourceAnnotation {
  return {
    id: 1,
    packagePath: "SimCityDataEP1.package",
    typeId: 0x00b1b104,
    groupId: 0x61f0c000,
    instance: 0x0302236c,
    topic: "欧米茄",
    title: "无人机产线-2",
    content: "车间顶棚",
    createdAt: 0,
    updatedAt: 0,
    ...overrides,
  };
}

describe("groupByTopic", () => {
  it("groups by topic, largest first, ties alphabetical", () => {
    const groups = groupByTopic([
      note({ id: 1, topic: "公园" }),
      note({ id: 2, topic: "欧米茄" }),
      note({ id: 3, topic: "公园" }),
      note({ id: 4, topic: "地标" }),
      note({ id: 5, topic: "欧米茄" }),
      note({ id: 6, topic: "欧米茄" }),
    ]);
    expect(groups.map((group) => group.topic)).toEqual([
      "欧米茄",
      "公园",
      "地标",
    ]);
    expect(groups[0].items).toHaveLength(3);
  });
});

describe("filterGroups", () => {
  const groups = groupByTopic([
    note({ id: 1, topic: "欧米茄", title: "无人机产线" }),
    note({ id: 2, topic: "欧米茄", title: "总部", content: "机器人群" }),
    note({ id: 3, topic: "公园", title: "湖畔", instance: 0xdeadbeef }),
  ]);

  it("keeps only groups with matches and filters their items", () => {
    const filtered = filterGroups(groups, "无人机");
    expect(filtered).toHaveLength(1);
    expect(filtered[0].topic).toBe("欧米茄");
    expect(filtered[0].items.map((item) => item.title)).toEqual(["无人机产线"]);
  });

  it("matches TGI text and topic names too", () => {
    // 仅湖畔用独立 instance：按 TGI 命中且把无命中组整组剔除
    expect(filterGroups(groups, "deadbeef").map((g) => g.topic)).toEqual([
      "公园",
    ]);
    expect(filterGroups(groups, "公园").map((g) => g.topic)).toEqual(["公园"]);
  });

  it("empty/blank query returns groups untouched (same reference)", () => {
    expect(filterGroups(groups, "  ")).toBe(groups);
  });
});

describe("tgi text helpers", () => {
  it("full text keeps three 8-digit segments; inline shows instance only", () => {
    const item = note({});
    expect(tgiTextOf(item)).toBe("00b1b104:61f0c000:0302236c");
    expect(tgiInstanceOf(item)).toBe("0302236c");
  });
});

describe("uniformPackageName", () => {
  it("returns basename when all items share one package", () => {
    expect(
      uniformPackageName([
        note({ packagePath: "a/SimCityDataEP1.package" }),
        note({ packagePath: "b\\SimCityDataEP1.package" }),
      ]),
    ).toBe("SimCityDataEP1.package");
  });
  it("returns null for mixed packages or empty items", () => {
    expect(
      uniformPackageName([
        note({ packagePath: "a.package" }),
        note({ packagePath: "b.package" }),
      ]),
    ).toBeNull();
    expect(uniformPackageName([])).toBeNull();
  });
});

describe("countResources", () => {
  it("deduplicates by TGI", () => {
    expect(
      countResources([note({ id: 1 }), note({ id: 2 }), note({ instance: 9 })]),
    ).toBe(2);
  });
});

describe("matchAnnotation", () => {
  it("is case-insensitive across fields", () => {
    expect(matchAnnotation(note({ title: "Drone Line" }), "drone")).toBe(true);
    expect(matchAnnotation(note({}), "nope")).toBe(false);
    expect(matchAnnotation(note({}), "")).toBe(true);
  });
});
