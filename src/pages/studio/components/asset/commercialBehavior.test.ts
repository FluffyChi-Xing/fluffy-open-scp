import { describe, expect, it } from "vitest";
import { commercialValues, validCommercial, MIXED_COMMERCIAL, type CommercialParams } from "./commercialBehavior";
import { normalizeLogicGraph } from "./logicGraph";

describe("commercial goods tiers", () => {
  it("defaults to more low-value goods while preserving the medium baseline", () => {
    expect(commercialValues({ type: "commercial", commercialProfile: MIXED_COMMERCIAL, goods: 450 }))
      .toMatchObject({ goodsW1: 900, goodsW2: 450, goodsW3: 225 });
    expect(commercialValues({ type: "commercial", commercialProfile: MIXED_COMMERCIAL }))
      .toMatchObject({ goodsW1: 1500, goodsW2: 750, goodsW3: 375 });
  });
  it("preserves legacy goods and independent overrides when reopening a saved graph", () => {
    const params: CommercialParams = { type: "commercial", commercialProfile: MIXED_COMMERCIAL,
      shoppersW1: 20, shoppersW2: 10, shoppersW3: 5,
      goods: 450, goodsW1: 120, goodsW3: 360 };
    const graph = normalizeLogicGraph({
      nodes: [{ id: "retail", kind: "behavior-leaf", position: [0, 0] }],
      edges: [], params: { retail: params },
    });
    expect(graph.params.retail).toMatchObject(params);
    expect(commercialValues(params)).toMatchObject({ goodsW1: 120, goodsW2: 450, goodsW3: 360 });
    expect(validCommercial(params)).toBe(true);
  });

  it("validates every authored tier without imposing a shared goods quantity", () => {
    const params: CommercialParams = { type: "commercial", commercialProfile: MIXED_COMMERCIAL,
      goodsW1: 1, goodsW2: 900, goodsW3: 100_000 };
    expect(validCommercial(params)).toBe(true);
    for (const invalid of [0, -1, 1.5, 100_001, Number.NaN]) {
      expect(validCommercial({ ...params, goodsW2: invalid })).toBe(false);
    }
  });
});
