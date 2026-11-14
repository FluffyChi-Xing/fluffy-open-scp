import { describe, expect, it } from "vitest";
import type { LotUnitDto } from "@/api/tauri";
import {
  CAMERA_ANCHOR_ID,
  UI_ANCHOR_ID,
  anchorKey,
  anchorKindOf,
  canUseAnchorName,
  describeSpawners,
  fnv1NameId,
  formatAnchorId,
  isAnchorInstanceValid,
  spawnerIssues,
  spawnerIssuesFor,
  suggestAnchorName,
  summarizeSpawners,
} from "./spawnerAnchor";

function spawner(
  index: number,
  instance: number | null,
  withTransform = true,
): LotUnitDto {
  return {
    kind: "spawner",
    index,
    transform: withTransform
      ? { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, index, 0, 0] }
      : null,
    id: instance === null ? null : anchorKey(instance),
    count: null,
    countRandom: null,
    agent: null,
    fields: [],
  } as unknown as LotUnitDto;
}

describe("fnv1NameId", () => {
  it("复现引擎 FNV-1（小写折叠）的名字 id", () => {
    // 与 database_main.s3db Instances 表逐字节一致的两条样本。
    expect(fnv1NameId("GI_Simm_politician_ma")).toBe(0x713a2388);
    expect(fnv1NameId("GI_Simm_polofficer_ma")).toBe(0x04bb64bd);
  });

  it("大小写折叠后等价", () => {
    expect(fnv1NameId("NorthDoor")).toBe(fnv1NameId("northdoor"));
  });
});

describe("anchorKindOf / formatAnchorId", () => {
  it("只有两个引擎硬编码 id 具备语义", () => {
    expect(anchorKindOf(UI_ANCHOR_ID)).toBe("ui");
    expect(anchorKindOf(CAMERA_ANCHOR_ID)).toBe("camera");
    expect(anchorKindOf(0x1234abcd)).toBe("custom");
    expect(anchorKindOf(null)).toBe("unnamed");
  });

  it("格式化为大写八位十六进制", () => {
    expect(formatAnchorId(UI_ANCHOR_ID)).toBe("0x141EED53");
    expect(formatAnchorId(null)).toBe("");
  });
});

describe("anchorKey / isAnchorInstanceValid", () => {
  it("名字 key 的 type/group 恒为 0（引擎的残缺 TGI 约定）", () => {
    expect(anchorKey(0x141eed53)).toEqual({
      typeId: 0,
      group: 0,
      instance: 0x141eed53,
    });
  });

  it("0 与非法值不算有效名字", () => {
    expect(isAnchorInstanceValid(0)).toBe(false);
    expect(isAnchorInstanceValid(null)).toBe(false);
    expect(isAnchorInstanceValid(-1)).toBe(false);
    expect(isAnchorInstanceValid(1)).toBe(true);
  });
});

describe("describeSpawners / summarizeSpawners", () => {
  it("分类、位置与重复标记", () => {
    const units = [
      spawner(0, UI_ANCHOR_ID),
      spawner(1, 0x11111111),
      spawner(2, 0x11111111),
      spawner(3, null),
    ];
    const anchors = describeSpawners(units);
    expect(anchors.map((anchor) => anchor.kind)).toEqual([
      "ui",
      "custom",
      "custom",
      "unnamed",
    ]);
    expect(anchors[0].label).toBe("0x141EED53");
    expect(anchors[1].duplicate).toBe(true);
    expect(anchors[0].duplicate).toBe(false);
    expect(anchors[0].position).toEqual([0, 0, 0]);
    expect(anchors[3].unitId).toBe("spawner:3");

    const summary = summarizeSpawners(units);
    expect(summary).toEqual({ total: 4, named: 3, unnamed: 1, duplicates: 2 });
  });
});

describe("spawnerIssues", () => {
  it("未命名 / 重复 / 缺变换三类问题都能报出", () => {
    const units = [
      spawner(0, null),
      spawner(1, 0x22222222),
      spawner(2, 0x22222222),
      spawner(3, 0x33333333, false),
    ];
    const issues = spawnerIssues(units);
    const codes = issues.map((issue) => issue.code).sort();
    expect(codes).toEqual([
      "anchor-duplicate",
      "anchor-duplicate",
      "anchor-missing",
      "transform-missing",
    ]);
    expect(
      spawnerIssuesFor(units, "spawner:2").map((issue) => issue.code),
    ).toEqual(["anchor-duplicate"]);
    expect(spawnerIssuesFor(units, "spawner:0")).toEqual([
      expect.objectContaining({ code: "anchor-missing" }),
    ]);
  });
});

describe("suggestAnchorName / canUseAnchorName", () => {
  it("建议名避开已占用的名字 id", () => {
    expect(suggestAnchorName([])).toBe("anchor");
    const taken = [spawner(0, fnv1NameId("anchor"))];
    expect(suggestAnchorName(taken)).toBe("anchor2");
  });

  it("重名检测忽略自身单元", () => {
    const units = [spawner(0, fnv1NameId("door"))];
    expect(canUseAnchorName(units, "door")).toBe(false);
    expect(canUseAnchorName(units, "door", "spawner:0")).toBe(true);
    expect(canUseAnchorName(units, "window")).toBe(true);
    expect(canUseAnchorName(units, "  ")).toBe(false);
  });
});
