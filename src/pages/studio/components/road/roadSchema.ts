import recipes from "./roadRecipes.json";
import { isRoadComposition, type RoadComposition } from "./roadComposition";
import { isRoadDraft, type RoadStrip } from "./roadDraft";

export { default as recipes } from "./roadRecipes.json";

export type RecipeId = keyof typeof recipes;
export const recipeIds = Object.keys(recipes) as RecipeId[];
export type PavedRecipe = Exclude<RecipeId, "dirt">;
export const pavedRecipes = recipeIds.filter(
  (recipe): recipe is PavedRecipe => recipe !== "dirt",
);

export const streetlightOptions = [
  "original",
  "left",
  "right",
  "none",
] as const;
export type Streetlights = (typeof streetlightOptions)[number];

export interface RoadSchema {
  format: "openscp.road-asset/1";
  recipe: RecipeId;
  identity: number;
  title: string;
  description: string;
  unitCost?: number | null;
  frontageEnabled?: boolean;
  gamePackage: string;
  scriptsPackage: string;
  streetlights?: Streetlights;
  composition?: RoadComposition;
  section?: RoadStrip[];
  baseRecipe?: PavedRecipe;
  menu?: RoadMenuSettings;
}

export interface RoadMenuSettings {
  position?: number | null;
  iconPng?: string;
  marqueePng?: string;
}

export function parseRoadSchema(content: string): RoadSchema {
  const value = JSON.parse(content);
  if (
    !value ||
    value.format !== "openscp.road-asset/1" ||
    !Object.hasOwn(recipes, value.recipe) ||
    (value.frontageEnabled !== undefined &&
      typeof value.frontageEnabled !== "boolean") ||
    (value.unitCost != null &&
      (!Number.isFinite(value.unitCost) ||
        !Number.isFinite(Math.fround(value.unitCost)) ||
        value.unitCost < 0)) ||
    (value.baseRecipe !== undefined &&
      !pavedRecipes.includes(value.baseRecipe)) ||
    (value.menu !== undefined &&
      (!value.menu ||
        typeof value.menu !== "object" ||
        Array.isArray(value.menu) ||
        (value.menu.position != null &&
          (!Number.isInteger(value.menu.position) ||
            value.menu.position < -2147483648 ||
            value.menu.position > 2147483647)) ||
        ["iconPng", "marqueePng"].some(
          (key) =>
            value.menu[key] !== undefined &&
            typeof value.menu[key] !== "string",
        ))) ||
    (value.section !== undefined && !isRoadDraft(value.section)) ||
    (value.composition !== undefined &&
      !isRoadComposition(value.composition, value.recipe)) ||
    (value.streetlights !== undefined &&
      !streetlightOptions.includes(value.streetlights)) ||
    (value.streetlights &&
      value.streetlights !== "original" &&
      value.recipe !== "highStreet") ||
    !Number.isInteger(value.identity) ||
    value.identity <= 0 ||
    value.identity > 0xffffffff ||
    ["title", "description", "gamePackage", "scriptsPackage"].some(
      (key) => typeof value[key] !== "string",
    )
  ) {
    throw new Error("road_schema_invalid");
  }
  return value;
}

export interface SectionPart {
  id: string;
  kind: "traffic" | "walk" | "median" | "rail" | "margin";
  width: number;
  direction?: "←" | "→";
}

export function crossSection(id: RecipeId): SectionPart[] {
  const recipe = recipes[id];
  const gap = Math.max(
    0,
    recipe.walkOffset - recipe.offset - recipe.lanes * recipe.width,
  );
  const traffic = (direction: "←" | "→"): SectionPart[] =>
    Array.from({ length: recipe.lanes }, (_, index) => ({
      id: `${direction}-${index}`,
      kind: "traffic",
      width: recipe.width,
      direction,
    }));
  return [
    { id: "walk-left", kind: "walk", width: recipe.walkWidth },
    ...(gap > 0
      ? [{ id: "margin-left", kind: "margin", width: gap } as const]
      : []),
    ...traffic("←"),
    ...(recipe.offset > 0
      ? [
          {
            id: "median",
            kind: id === "streetcar" ? "rail" : "median",
            width: recipe.offset * 2,
          } as const,
        ]
      : []),
    ...traffic("→"),
    ...(gap > 0
      ? [{ id: "margin-right", kind: "margin", width: gap } as const]
      : []),
    { id: "walk-right", kind: "walk", width: recipe.walkWidth },
  ];
}

export function recipeDraft(recipe: RecipeId): RoadStrip[] {
  return crossSection(recipe).map(({ id, kind, direction, width }) => ({
    id,
    kind:
      kind === "traffic" ? (direction === "←" ? "backward" : "forward") : kind,
    width: Number(width.toFixed(9)),
  }));
}

export function matchingRecipe(
  section: readonly RoadStrip[],
): RecipeId | undefined {
  return recipeIds.find((recipe) => {
    const template = recipeDraft(recipe);
    return (
      template.length === section.length &&
      template.every(
        (part, index) =>
          part.kind === section[index].kind &&
          Math.abs(part.width - section[index].width) < 1e-8,
      )
    );
  });
}
