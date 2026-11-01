import { describe, expect, it } from "vitest";
import type { LightUnit, LotUnitDto } from "@/api/tauri";
import { fnv1NameId } from "@/pages/packages/components/unit-inspector/spawnerAnchor";
import { useUnitClipboard, type UnitClipboardHost } from "./useUnitClipboard";

const lightUnit = (index: number): LightUnit => ({
  kind: "light",
  index,
  transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 5, 0, 6] },
  lightType: "Point",
  color: [1, 1, 1],
  outerRadius: 8,
  innerRadius: null,
  diffuse: null,
  length: null,
  cullDistance: "Far",
  isVolumetric: false,
  debugName: null,
  fields: [],
});

const pathPointUnit = (index: number): LotUnitDto =>
  ({
    kind: "pathPoint",
    index,
    point: [0, 0, 0],
    tangent: null,
    pointIndex: index,
    fields: [],
  }) as unknown as LotUnitDto;

/** 假宿主：addedUnits/deletedIds 语义简化为纯数组增删 + 选中记录。 */
function fakeHost(initial: LotUnitDto[] = []): UnitClipboardHost & {
  added: LotUnitDto[];
  selected: (string | null)[];
} {
  const added: LotUnitDto[] = [];
  const selected: (string | null)[] = [];
  const units = [...initial];
  return {
    added,
    selected,
    get selectedUnit() {
      return (
        units.find(
          (unit) => `${unit.kind}:${unit.index}` === selected.at(-1),
        ) ?? null
      );
    },
    get units() {
      return [...units, ...added];
    },
    addUnit(unit) {
      added.push(unit);
    },
    selectUnit(id) {
      selected.push(id);
    },
  };
}

describe("useUnitClipboard", () => {
  it("copy → paste adds a re-indexed duplicate and selects it", () => {
    const host = fakeHost([lightUnit(0)]);
    host.selectUnit("light:0");
    const clipboard = useUnitClipboard(host);
    expect(clipboard.copy()).toBe("copied");
    expect(clipboard.paste()).toBe("pasted");
    expect(host.added).toHaveLength(1);
    const clone = host.added[0] as LightUnit;
    expect(clone.index).toBe(1);
    expect(clone.transform?.matrix.slice(9)).toEqual([5, 0, 6]);
    expect(host.selected.at(-1)).toBe("light:1");
  });

  it("paste is repeatable (spec untouched by paste)", () => {
    const host = fakeHost([lightUnit(0)]);
    host.selectUnit("light:0");
    const clipboard = useUnitClipboard(host);
    clipboard.copy();
    clipboard.paste();
    clipboard.paste();
    expect(host.added.map((unit) => unit.index)).toEqual([1, 2]);
  });

  it("refuses to copy pathPoint", () => {
    const host = fakeHost([pathPointUnit(0)]);
    host.selectUnit("pathPoint:0");
    const clipboard = useUnitClipboard(host);
    expect(clipboard.copy()).toBe("blocked-path-point");
    expect(clipboard.paste()).toBe("empty-clipboard");
  });

  it("copy without selection is a no-op; paste without clipboard too", () => {
    const host = fakeHost();
    const clipboard = useUnitClipboard(host);
    expect(clipboard.copy()).toBe("no-selection");
    expect(clipboard.paste()).toBe("empty-clipboard");
    expect(host.added).toHaveLength(0);
  });

  it("粘贴生成器锚点时改名为不重复的新名字（引擎按名字建表）", () => {
    const original = {
      kind: "spawner",
      index: 0,
      transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 1] },
      id: { typeId: 0, group: 0, instance: fnv1NameId("anchor") },
      count: null,
      countRandom: null,
      agent: null,
      fields: [],
    } as unknown as LotUnitDto;
    const host = fakeHost([original]);
    host.selectUnit("spawner:0");
    const clipboard = useUnitClipboard(host);
    expect(clipboard.copy()).toBe("copied");
    expect(clipboard.paste()).toBe("pasted");

    const clone = host.added[0] as unknown as {
      index: number;
      id: { typeId: number; group: number; instance: number };
    };
    expect(clone.index).toBe(1);
    expect(clone.id.instance).not.toBe(fnv1NameId("anchor"));
    expect(clone.id).toMatchObject({ typeId: 0, group: 0 });
    expect(clone.id.instance).toBe(fnv1NameId("anchor2"));
  });
});
