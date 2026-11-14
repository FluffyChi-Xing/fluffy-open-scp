import { OFFICE_FIELDS } from "./officeBehavior";

export const COMMERCIAL_DEFAULTS = { w1: 3, w2: 2, w3: 1, minWorkers: 3, shoppers: 9, goods: 9 } as const;
export const COMMERCIAL_FIELDS = [...OFFICE_FIELDS, "shoppers", "goods"] as const;
export const COMMERCIAL_PROFILES = {
  "ground-commercial-w1-v1": { w1: 5, w2: 2, w3: 0, minWorkers: 3, shoppers: 10, goods: 21 },
  "ground-commercial-w2-v1": { w1: 3, w2: 2, w3: 1, minWorkers: 3, shoppers: 8, goods: 18 },
  "ground-commercial-w3-v1": { w1: 1, w2: 2, w3: 2, minWorkers: 3, shoppers: 7, goods: 15 },
} as const;
export type CommercialProfile = keyof typeof COMMERCIAL_PROFILES;

export type CommercialParams = {
  type: "commercial";
  commercialProfile?: CommercialProfile;
} & Partial<Record<(typeof COMMERCIAL_FIELDS)[number], number | null>>;

export function commercialDefaults(profile?: CommercialProfile) {
  return profile ? COMMERCIAL_PROFILES[profile] : COMMERCIAL_DEFAULTS;
}

export function validCommercial(params: CommercialParams): boolean {
  const defaults = commercialDefaults(params.commercialProfile);
  if (!defaults) return false;
  const [w1, w2, w3, minimum, shoppers, goods] = COMMERCIAL_FIELDS.map(
    (field) => params[field] ?? defaults[field],
  );
  const total = w1 + w2 + w3;
  return [w1, w2, w3, minimum, shoppers, goods].every(Number.isInteger) &&
    w1 >= 0 && w2 >= 0 && w3 >= 0 && total <= 100_000 &&
    minimum >= 1 && minimum <= total &&
    shoppers >= 1 && shoppers <= 100_000 && goods >= 1 && goods <= 100_000;
}
