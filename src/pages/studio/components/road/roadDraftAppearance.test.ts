import { expect, it } from "vitest";
import type { RoadAppearance } from "@/pages/packages/components/preview/roadAppearanceScene";
import type { RoadRibbon } from "@/lib/region-map";
import { composeRoadDraftAppearance } from "./roadDraftAppearance";
import { pavedRecipes, parseRoadSchema } from "./roadSchema";

const ribbon = (
  component: number,
  x: number,
  width: number,
  texture = component,
): RoadRibbon => ({
  component,
  offset: [x, 0, 0],
  worldSize: [width, 4],
  scale: [1, 1, 1],
  uvStart: [0, 0],
  uvEnd: [1, 1],
  texture,
});

it("lays out actual donor ribbons at authored widths and retains repeated features", () => {
  const assets: RoadAppearance = {
    instances: [],
    models: {},
    sweeps: {},
    unsupportedComponents: [],
    textures: { 4: "road", 5: "walk", 6: "median", 7: "track-bed", 8: "rail" },
    ribbons: {
      20: [ribbon(4, 3, 6), ribbon(5, -9, 6)],
      1: [ribbon(6, 0, 8)],
      2: [ribbon(7, 0, 12), ribbon(8, 3, 1)],
    },
  };
  const output = composeRoadDraftAppearance(20, assets, "mediumStreet", [
    { id: "walk", kind: "walk", width: 2 },
    { id: "median-a", kind: "median", width: 1 },
    { id: "lane", kind: "forward", width: 3 },
    { id: "rail", kind: "rail", width: 6 },
    { id: "median-b", kind: "median", width: 2 },
  ]);
  expect(
    output.ribbons[20].map((part) => [part.texture, part.offset[0]]),
  ).toEqual([
    [5, -6],
    [6, -4.5],
    [4, -2.5],
    [7, 2],
    [8, 3.5],
    [6, 6],
  ]);
  expect(output.ribbons[20][2].uvStart[0]).toBe(0.5);
  expect(output.ribbons[20][3].scale[0]).toBe(0.5);
  expect(output.unsupportedComponents).toEqual([]);
  expect(assets.ribbons[20][0].worldSize[0]).toBe(6);
});

it("allows only paved recipes as an explicit draft base", () => {
  const schema = {
    format: "openscp.road-asset/1",
    recipe: "highStreet",
    identity: 1,
    title: "Road",
    description: "",
    gamePackage: "",
    scriptsPackage: "",
    section: [],
  };
  expect(pavedRecipes).not.toContain("dirt");
  expect(
    parseRoadSchema(JSON.stringify({ ...schema, baseRecipe: "lowStreet" }))
      .baseRecipe,
  ).toBe("lowStreet");
  expect(() =>
    parseRoadSchema(JSON.stringify({ ...schema, baseRecipe: "dirt" })),
  ).toThrow("road_schema_invalid");
});
