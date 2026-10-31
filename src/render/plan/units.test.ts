import type { LotUnitDto } from "@/api/tauri";
import { describe, expect, it } from "vitest";
import type { UnitPlanContext } from "../contract/types";
import { directAssetKey, planUnit, unitPlanId } from "./units";

const identityMatrix = [1, 0, 0, 0, 1, 0, 0, 0, 1, 2, 3, 4];

function prop(overrides: Record<string, unknown> = {}): LotUnitDto {
  return {
    kind: "prop",
    index: 0,
    bin: 0,
    resourceId: 100,
    transform: { matrix: identityMatrix },
    scale: 2,
    fields: [],
    ...overrides,
  } as unknown as LotUnitDto;
}

function spawner(): LotUnitDto {
  return {
    kind: "spawner",
    index: 3,
    transform: { matrix: identityMatrix },
    fields: [],
  } as unknown as LotUnitDto;
}

function context(overrides: Partial<UnitPlanContext> = {}): UnitPlanContext {
  return {
    renderMode: "refined",
    treeIds: new Set(),
    propModels: new Map([[100, {}]]),
    treeModelPayloadCount: 0,
    simPartsCount: 0,
    ...overrides,
  };
}

describe("planUnit 分发语义（与旧 renderers 对齐）", () => {
  it("默认模式一律不适用（退标记锥）", () => {
    expect(planUnit(prop(), context({ renderMode: "default" }))).toBeNull();
  });

  it("prop：resourceId 有载荷 → prop 计划（resource 资产 + dtoScale）", () => {
    const plan = planUnit(prop(), context());
    expect(plan?.kind).toBe("prop");
    if (plan?.kind !== "prop") return;
    expect(plan.asset).toEqual({ kind: "resource", resourceId: 100 });
    expect(plan.dtoScale).toBe(2);
    expect(plan.transform?.position).toEqual([2, 3, 4]);
    expect(plan.scale).toEqual({ kind: "baked", factor: 2 });
  });

  it("prop：resourceId 缺载荷 → null", () => {
    expect(planUnit(prop(), context({ propModels: new Map() }))).toBeNull();
  });

  it("树：treeIds 命中优先于 prop，seed 采用旧公式", () => {
    const plan = planUnit(prop(), context({ treeIds: new Set([100]) }));
    expect(plan?.kind).toBe("tree");
    if (plan?.kind !== "tree") return;
    expect(plan.source).toBe("resolved");
    expect(plan.seed).toBe(100 * 2654435761 + 0);
    // 无 3D 形状载荷 → 公告板尺寸自持
    expect(plan.scale).toEqual({ kind: "intrinsic" });
  });

  it("树：有形状载荷 → baked 缩放语义", () => {
    const plan = planUnit(
      prop(),
      context({ treeIds: new Set([100]), treeModelPayloadCount: 4 }),
    );
    expect(plan?.kind).toBe("tree");
    expect(plan?.scale).toEqual({ kind: "baked", factor: 2 });
  });

  it("直挂模型树：assetCategory 命中 trees → direct 树计划", () => {
    const modelTgi = { typeId: 1, group: 0, instance: 0x4df43690 };
    const unit = prop({ resourceId: undefined, modelTgi, modelPackageId: 3, displayName: "tree" });
    const key = directAssetKey({ modelTgi, modelPackageId: 3 });
    const plan = planUnit(unit, context({ addedModelPayloads: new Map([[key, {}]]) }));
    expect(plan?.kind).toBe("tree");
    if (plan?.kind !== "tree") return;
    expect(plan.source).toBe("direct");
    expect(plan.asset).toEqual({ kind: "instance", key });
    expect(plan.seed).toBe(0);
  });

  it("直挂模型非树 → prop 计划（instance 资产）", () => {
    const modelTgi = { typeId: 1, group: 0, instance: 0x12345678 };
    const unit = prop({ resourceId: undefined, modelTgi, modelPackageId: 3, displayName: "bench" });
    const key = directAssetKey({ modelTgi, modelPackageId: 3 });
    const plan = planUnit(unit, context({ addedModelPayloads: new Map([[key, {}]]) }));
    expect(plan?.kind).toBe("prop");
    if (plan?.kind !== "prop") return;
    expect(plan.asset).toEqual({ kind: "instance", key });
  });

  it("直挂模型缺载荷时不回退 propModels（旧语义）", () => {
    const modelTgi = { typeId: 1, group: 0, instance: 0x12345678 };
    const unit = prop({ modelTgi, modelPackageId: 3 });
    expect(planUnit(unit, context())).toBeNull();
  });

  it("spawner：无小人资产 → null；有资产 → intrinsic", () => {
    expect(planUnit(spawner(), context({ simPartsCount: 0 }))).toBeNull();
    const plan = planUnit(spawner(), context({ simPartsCount: 2 }));
    expect(plan?.kind).toBe("spawner");
    expect(plan?.scale).toEqual({ kind: "intrinsic" });
  });

  it("稳定 id 与旧 unitId 一致", () => {
    expect(unitPlanId(prop({ bin: 3, index: 7 }))).toBe("prop:3:7");
  });

  it("同一输入的计划是纯数据（可 JSON 序列化）", () => {
    const plan = planUnit(prop(), context());
    expect(JSON.parse(JSON.stringify(plan))).toEqual(plan);
  });
});
