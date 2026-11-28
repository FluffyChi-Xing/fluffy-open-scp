import { describe, it, expect } from "vitest";
import { selectActorLook } from "./actorLook";
import { bakeActorTexture, type Pixels } from "./actorVisualAssets";
import type { ActorInfo } from "@/api/property-semantics";
const actor: ActorInfo = {
  id: 1,
  models: [],
  templates: [],
  appearances: [
    { body: null, skin: -1, skinMax: -1, outfit: -1, outfitMax: -1 },
  ],
  speed: 5,
  scale: 1,
  animation: null,
  spawn: null,
  destroy: null,
  resourceBind: null,
};
describe("native actor appearance", () => {
  it("flattens inclusive body, head and outfit ranges in engine order", () => {
    const parts = {
      bodies: [0],
      bodiesMax: [1],
      heads: [2],
      headsMax: [3],
      outfits: [64],
      outfitsMax: [65],
    };
    expect(selectActorLook(parts, actor, 7 << 12)).toMatchObject({
      body: 1,
      head: 3,
      outfit: 65,
    });
    for (let i = 0; i < 32; i++)
      expect(
        selectActorLook(parts, actor, i << 12).outfit,
      ).toBeGreaterThanOrEqual(64);
  });
  it("uses actor overrides while treating -1 as an unspecified range", () => {
    const parts = {
      bodies: [0],
      bodiesMax: [0],
      heads: [0],
      headsMax: [0],
      outfits: [32],
      outfitsMax: [63],
    };
    const a = {
      ...actor,
      appearances: [
        { body: 2, skin: 4, skinMax: 7, outfit: 80, outfitMax: 95 },
      ],
    };
    const look = selectActorLook(parts, a, 12345678);
    expect(look.body).toBe(2);
    expect(look.skin).toBeGreaterThanOrEqual(4);
    expect(look.skin).toBeLessThanOrEqual(7);
    expect(look.outfit).toBeGreaterThanOrEqual(80);
    expect(look.outfit).toBeLessThanOrEqual(95);
  });
  it("applies AO, palette columns and the clothing alpha mask", () => {
    const palette = (color: number[]): Pixels => ({
      width: 256,
      height: 32,
      rgba: Array.from({ length: 256 * 32 }, () => color).flat(),
    });
    const source = { width: 1, height: 1, rgba: [128, 128, 0, 200] };
    const rgba = bakeActorTexture(
      source,
      palette([200, 100, 50, 255]),
      palette([20, 40, 80, 255]),
      { skin: 0, outfit: 64 },
    );
    expect([...rgba]).toEqual([10, 20, 40, 200]);
    expect([
      ...bakeActorTexture(
        source,
        palette([200, 100, 50, 255]),
        palette([20, 40, 80, 0]),
        { skin: 0, outfit: 64 },
      ),
    ]).toEqual([100, 50, 25, 200]);
  });
});
