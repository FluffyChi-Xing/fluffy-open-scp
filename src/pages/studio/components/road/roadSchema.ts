export const recipes = {
  dirt: {
    lanes: 1,
    width: 4.5,
    offset: 0.5,
    walkWidth: 4,
    walkOffset: 8,
    surface: "gravel",
  },
  lowStreet: {
    lanes: 1,
    width: 4.5,
    offset: 0,
    walkWidth: 3,
    walkOffset: 9,
    surface: "asphalt",
  },
  mediumStreet: {
    lanes: 2,
    width: 3.2,
    offset: 0,
    walkWidth: 3,
    walkOffset: 9,
    surface: "asphalt",
  },
  highStreet: {
    lanes: 2,
    width: 4.4,
    offset: 0,
    walkWidth: 3,
    walkOffset: 9,
    surface: "asphalt",
  },
  mediumAvenue: {
    lanes: 2,
    width: 5.45,
    offset: 4.5,
    walkWidth: 3,
    walkOffset: 21.15,
    surface: "asphalt",
  },
  highAvenue: {
    lanes: 3,
    width: 5.45,
    offset: 4,
    walkWidth: 3,
    walkOffset: 21.15,
    surface: "asphalt",
  },
  streetcar: {
    lanes: 3,
    width: 4.5,
    offset: 6,
    walkWidth: 2.85,
    walkOffset: 21.15,
    surface: "asphalt",
  },
} as const;

export type RecipeId = keyof typeof recipes;
export const recipeIds = Object.keys(recipes) as RecipeId[];

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
  gamePackage: string;
  scriptsPackage: string;
  streetlights?: Streetlights;
}

export function parseRoadSchema(content: string): RoadSchema {
  const value = JSON.parse(content);
  if (
    !value ||
    value.format !== "openscp.road-asset/1" ||
    !Object.hasOwn(recipes, value.recipe) ||
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
