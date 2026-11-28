export const GROUND_RESIDENCE_DEFAULTS = {
  "ground-residence-w1-v1": { population: 60, workers: 40, children: 15 },
  "ground-residence-w1-d3-v1": { population: 600, workers: 400, children: 100 },
  "ground-residence-w2-v1": { population: 30, workers: 20, children: 10 },
  "ground-residence-w3-v1": { population: 10, workers: 7, children: 4 },
} as const;

export type GroundResidenceProfile = keyof typeof GROUND_RESIDENCE_DEFAULTS;

export interface ResidenceParams {
  type: "residence";
  residenceProfile?: "copan-residence-v1" | "megatower-residence-v1" | GroundResidenceProfile;
  population?: number | null;
  residentWorkers?: number | null;
  residentShoppers?: number | null;
  residentChildren?: number | null;
  happinessInitial?: number | null;
  happinessCapacity?: number | null;
}

export const RESIDENCE_CAPACITY_FIELDS = ["residentWorkers", "residentShoppers", "residentChildren"] as const;
export const RESIDENCE_HAPPINESS_FIELDS = ["happinessInitial", "happinessCapacity"] as const;

export function groundResidence(params: ResidenceParams) {
  const profile = params.residenceProfile;
  return profile && Object.hasOwn(GROUND_RESIDENCE_DEFAULTS, profile)
    ? GROUND_RESIDENCE_DEFAULTS[profile as GroundResidenceProfile]
    : null;
}

export function residenceCapacityError(params: ResidenceParams) {
  const { population, residentWorkers, residentShoppers, happinessInitial, happinessCapacity } = params;
  const defaults = groundResidence(params);
  const within = (value: number | null | undefined, min: number, max: number) =>
    value == null || (Number.isInteger(value) && value >= min && value <= max);

  if (!within(population, 1, 100_000)) return "populationInvalid";
  if (!defaults) {
    return [...RESIDENCE_CAPACITY_FIELDS, ...RESIDENCE_HAPPINESS_FIELDS].some(
      (field) => params[field] != null,
    ) ? "residentSlotsInvalid" : null;
  }
  const capacity = population ?? defaults.population;
  if (!RESIDENCE_CAPACITY_FIELDS.every((field) => within(params[field], 0, capacity))
    || (residentWorkers != null && residentShoppers != null && residentWorkers + residentShoppers !== capacity)) {
    return "residentSlotsInvalid";
  }
  if (!within(happinessCapacity, 1, 1_100_000)
    || !within(happinessInitial, 0, happinessCapacity ?? capacity * 11)) {
    return "residentHappinessInvalid";
  }
  return null;
}
