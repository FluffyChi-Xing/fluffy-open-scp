import type { RecipeId } from "./roadSchema";

export const roadComponents = ["leftTrees", "rightTrees", "tram"] as const;
export type RoadComponent = (typeof roadComponents)[number];
export type RoadComposition = Partial<Record<RoadComponent, boolean>>;
export const roadComponentDragType = "application/x-openscp-road-component";

export function availableComponents(
  recipe: RecipeId,
): readonly RoadComponent[] {
  if (recipe === "mediumStreet") return roadComponents;
  if (recipe === "streetcar") return ["leftTrees", "rightTrees"];
  return [];
}

export function isRoadComposition(
  value: unknown,
  recipe: RecipeId,
): value is RoadComposition {
  return (
    !!value &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    Object.entries(value).every(
      ([key, enabled]) =>
        roadComponents.includes(key as RoadComponent) &&
        typeof enabled === "boolean" &&
        (!enabled ||
          availableComponents(recipe).includes(key as RoadComponent)),
    )
  );
}
