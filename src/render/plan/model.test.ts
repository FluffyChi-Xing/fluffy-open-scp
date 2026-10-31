import { describe, expect, it } from "vitest";
import type { ModelPlan, ModelMeshFacts } from "./model";
import { planMeshMaterial, planMeshMaterials, planModel } from "./model";

const refinedPlan: ModelPlan = planModel({
  renderMode: "refined",
  modelSource: "native",
  reliefEnabled: false,
  modelSelectable: true,
  modelObjectCount: 2,
  payloadMaterialCount: 3,
  hasPayload: true,
  modelOrigin: [1, 2, 0],
  modelTransform: null,
});

const mesh = (overrides: Partial<ModelMeshFacts> = {}): ModelMeshFacts => ({
  materialIndex: 0,
  uvKind: 2,
  hasVertexColor: false,
  importedMaterials: [{ name: "wall", hasMap: false, hasNormalMap: false, hasEmissiveMap: false }],
  ...overrides,
});

const facts = (overrides: Record<string, unknown> = {}) => ({
  plan: refinedPlan,
  materialIndex: 0,
  materialName: "wall",
  mesh: mesh(),
  tintReady: [{ hasTint: true }],
  payloadMaterialNames: ["wall", "roof", "glass"],
  tintPaletteRows: [new Map<string, number>()],
  ...overrides,
});

describe("planModel 顶层开关", () => {
  it("精细 + 原生载荷 → tint/deferred 开启，白模关闭", () => {
    expect(refinedPlan).toMatchObject({
      refined: true,
      tintEnabled: true,
      deferredEnabled: true,
      whiteMode: false,
      wrapModelUnit: true,
      shadows: true,
    });
  });

  it("默认模式 → 白模且不编译 tint/deferred", () => {
    const plan = planModel({
      renderMode: "default",
      modelSource: "native",
      reliefEnabled: true,
      modelSelectable: false,
      modelObjectCount: 3,
      payloadMaterialCount: 2,
      hasPayload: true,
      modelOrigin: null,
      modelTransform: null,
    });
    expect(plan).toMatchObject({
      whiteMode: true,
      tintEnabled: false,
      deferredEnabled: false,
      shadows: false,
      wrapModelUnit: false,
    });
  });

  it("GLB 无六槽材质 → 不开 tint；transform 长度非法 → null", () => {
    const plan = planModel({
      renderMode: "refined",
      modelSource: "glb",
      reliefEnabled: false,
      modelSelectable: false,
      modelObjectCount: 1,
      payloadMaterialCount: 0,
      hasPayload: true,
      modelOrigin: null,
      modelTransform: [1, 2, 3],
    });
    expect(plan.tintEnabled).toBe(false);
    expect(plan.transform).toBeNull();
  });
});

describe("planMeshMaterial 分支", () => {
  it("默认模式恒白模", () => {
    const plan = planModel({
      renderMode: "default",
      modelSource: "native",
      reliefEnabled: false,
      modelSelectable: false,
      modelObjectCount: 1,
      payloadMaterialCount: 1,
      hasPayload: true,
      modelOrigin: null,
      modelTransform: null,
    });
    expect(planMeshMaterial(facts({ plan }))).toEqual({ kind: "white" });
  });

  it("uvKind 2 + tint 命中 → tint 着色链（palette 0，非 Blender）", () => {
    expect(planMeshMaterial(facts())).toEqual({
      kind: "tint",
      tintIndex: 0,
      materialName: undefined,
      blenderUvProfile: false,
      paletteRow: 0,
      blenderAtlasUv: false,
    });
  });

  it("uvKind 2 + tint 缺失 → refined（vertexColors 跟随几何）", () => {
    const strategy = planMeshMaterial(
      facts({ tintReady: [{ hasTint: false }], mesh: mesh({ hasVertexColor: true }) }),
    );
    expect(strategy).toEqual({
      kind: "refined",
      vertexColors: true,
      materialIndex: 0,
    });
  });

  it("uvKind 3：按材质名反查材质列并取调色板行", () => {
    const strategy = planMeshMaterial(
      facts({
        materialName: "roof",
        mesh: mesh({ uvKind: 3 }),
        tintReady: [{ hasTint: true }, { hasTint: true }, { hasTint: true }],
        tintPaletteRows: [new Map(), new Map([["roof", 4]])],
      }),
    );
    expect(strategy).toMatchObject({
      kind: "tint",
      tintIndex: 1,
      materialName: "roof",
      blenderUvProfile: true,
      paletteRow: 4,
      blenderAtlasUv: true,
    });
  });

  it("uvKind 3 whitebox/roof 特例：palette 9.6 + atlas UV", () => {
    const strategy = planMeshMaterial(
      facts({
        materialName: "whitebox",
        mesh: mesh({ uvKind: 3 }),
      }),
    );
    expect(strategy).toMatchObject({ kind: "tint", paletteRow: 9.6, blenderAtlasUv: true });
  });

  it("GLB 无六槽材质：贴图优先保留，其次顶点色，最后白模", () => {
    const plan = planModel({
      renderMode: "refined",
      modelSource: "glb",
      reliefEnabled: false,
      modelSelectable: false,
      modelObjectCount: 1,
      payloadMaterialCount: 0,
      hasPayload: true,
      modelOrigin: null,
      modelTransform: null,
    });
    const textured = planMeshMaterial(
      facts({
        plan,
        payloadMaterialNames: [],
        mesh: mesh({
          importedMaterials: [
            { name: "a", hasMap: true, hasNormalMap: false, hasEmissiveMap: false },
          ],
        }),
      }),
    );
    expect(textured).toEqual({ kind: "keepImported" });
    const colored = planMeshMaterial(
      facts({
        plan,
        payloadMaterialNames: [],
        mesh: mesh({
          hasVertexColor: true,
          importedMaterials: [
            { name: "a", hasMap: false, hasNormalMap: false, hasEmissiveMap: false },
          ],
        }),
      }),
    );
    expect(colored).toEqual({ kind: "vertexColor" });
    const bare = planMeshMaterial(
      facts({
        plan,
        payloadMaterialNames: [],
        mesh: mesh({
          importedMaterials: [
            { name: "a", hasMap: false, hasNormalMap: false, hasEmissiveMap: false },
          ],
        }),
      }),
    );
    expect(bare).toEqual({ kind: "white" });
  });
});

describe("planMeshMaterials 数组/延迟贴图组", () => {
  it("uvKind=1 的 refined 材质进入 deferred 组", () => {
    const result = planMeshMaterials({
      plan: refinedPlan,
      materialIndex: 2,
      mesh: mesh({ uvKind: 1, importedMaterials: [] }),
      importedNames: ["wall"],
      tintReady: [{ hasTint: false }],
      payloadMaterialNames: ["wall"],
      tintPaletteRows: [new Map()],
    });
    expect(result.strategies).toEqual([
      { kind: "refined", vertexColors: false, materialIndex: 2 },
    ]);
    expect(result.deferredGroupMaterialIndexes).toEqual([2]);
  });
});
