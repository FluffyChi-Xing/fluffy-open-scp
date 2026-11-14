import { describe, expect, it } from "vitest";
import type { LightUnit, LotUnitDto, PropUnit } from "@/api/tauri";
import { captureUnitSpec, nextIndexOfKind, specToUnit } from "./unitClipboard";

const lightUnit = (index: number): LightUnit => ({
  kind: "light",
  index,
  transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 3, 0, 4] },
  lightType: "Spot",
  color: [1, 0.5, 0.25],
  outerRadius: 12,
  innerRadius: 2,
  diffuse: null,
  length: null,
  cullDistance: "Mid",
  isVolumetric: true,
  debugName: "门灯",
  fields: [{ hash: 1, typeName: "float", value: "0.5" }],
});

const propUnit = (index: number): PropUnit => ({
  kind: "prop",
  displayName: "长椅",
  index,
  bin: 2,
  resourceId: 0x1234,
  transform: { matrix: [2, 0, 0, 0, 2, 0, 0, 0, 2, 8, 0, -3] },
  slot: null,
  scale: 1.5,
  fields: [],
  modelTgi: { typeId: 0x2f4e681b, group: 0, instance: 0xabcd },
  modelPackageId: 7,
  modelPackagePath: "models/bench.glb",
});

const pathPointUnit = (index: number): LotUnitDto =>
  ({
    kind: "pathPoint",
    index,
    point: [1, 0, 2],
    tangent: null,
    pointIndex: index,
    fields: [],
  }) as unknown as LotUnitDto;

describe("captureUnitSpec", () => {
  it("captures light as a detached deep clone", () => {
    const unit = lightUnit(3);
    const spec = captureUnitSpec(unit) as LightUnit;
    expect(spec.kind).toBe("light");
    // 规格与源隔离：改规格不得影响原单元
    spec.outerRadius = 99;
    expect(unit.outerRadius).toBe(12);
  });

  it("captures prop with model reference fields intact", () => {
    const spec = captureUnitSpec(propUnit(1));
    expect(spec).not.toBeNull();
    const prop = spec as PropUnit;
    expect(prop.modelTgi?.instance).toBe(0xabcd);
    expect(prop.modelPackagePath).toBe("models/bench.glb");
    expect(prop.bin).toBe(2);
    expect(prop.displayName).toBe("长椅");
  });

  it("rejects pathPoint (only created while drawing a path)", () => {
    expect(captureUnitSpec(pathPointUnit(0))).toBeNull();
  });
});

describe("specToUnit", () => {
  it("rebuilds with the given index and preserves everything else", () => {
    const source = lightUnit(3);
    const spec = captureUnitSpec(source)!;
    const unit = specToUnit(spec, 9) as LightUnit;
    expect(unit.index).toBe(9);
    expect(unit.lightType).toBe("Spot");
    expect(unit.transform?.matrix).toEqual(source.transform?.matrix);
    expect(unit.fields).toEqual(source.fields);
    expect(unit.isVolumetric).toBe(true);
  });

  it("does not mutate the spec (paste stays repeatable)", () => {
    const spec = captureUnitSpec(lightUnit(3)) as LightUnit;
    specToUnit(spec, 9);
    expect(spec.index).toBe(3);
    expect(spec.outerRadius).toBe(12);
  });
});

describe("nextIndexOfKind", () => {
  it("returns 0 for an empty unit set", () => {
    expect(nextIndexOfKind([], "light")).toBe(0);
  });

  it("takes max index of the same kind only", () => {
    const units = [lightUnit(4), propUnit(7), pathPointUnit(100)];
    expect(nextIndexOfKind(units, "light")).toBe(5);
    expect(nextIndexOfKind(units, "prop")).toBe(8);
  });
});
