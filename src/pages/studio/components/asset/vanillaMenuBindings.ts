import catalogue from "@/lib/game-ui/vanilla-building-bindings.json";

interface BuildingBinding {
  unit: string;
  lot: string;
  primary: boolean;
  matched: boolean;
}

// Retail menu properties from Scripts build 287520926 and its stock SimCityData packages.
const bindings: Partial<Record<string, BuildingBinding[]>> = catalogue;

export function vanillaMenuBindings(id: string): BuildingBinding[] | undefined {
  return bindings[id.toUpperCase().replace(/^0X/, "")];
}

export function vanillaMenuIdsForUnit(unit: string): string[] {
  const normalized = unit.toUpperCase();
  return Object.entries(bindings)
    .filter(([, choices]) => choices?.some((choice) => choice.unit === normalized))
    .map(([id]) => id);
}
