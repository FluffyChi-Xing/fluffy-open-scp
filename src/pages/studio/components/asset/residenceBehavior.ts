export const GROUND_RESIDENCE_DEFAULTS = {
  "ground-residence-w1-v1": { population: 60, workers: 40, children: 15 },
  "ground-residence-w2-v1": { population: 30, workers: 20, children: 10 },
  "ground-residence-w3-v1": { population: 10, workers: 7, children: 4 },
} as const;

export type GroundResidenceProfile = keyof typeof GROUND_RESIDENCE_DEFAULTS;

export interface ResidenceParams {
  type: "residence";
  residenceProfile?: "copan-residence-v1" | "megatower-residence-v1" | GroundResidenceProfile;
  population?: number | null;
  residentWorkers?: number | null;
  residentChildren?: number | null;
}

export function groundResidence(params: ResidenceParams) {
  const profile = params.residenceProfile;
  return profile && Object.hasOwn(GROUND_RESIDENCE_DEFAULTS, profile)
    ? GROUND_RESIDENCE_DEFAULTS[profile as GroundResidenceProfile]
    : null;
}

export function residenceCapacityError(params: ResidenceParams) {
  const { population, residentWorkers, residentChildren } = params;
  const defaults = groundResidence(params);
  const within = (value: number | null | undefined, min: number, max: number) =>
    value == null || (Number.isInteger(value) && value >= min && value <= max);

  if (!within(population, 1, 100_000)) return "populationInvalid";
  if (!defaults) {
    return residentWorkers != null || residentChildren != null ? "residentSlotsInvalid" : null;
  }
  const capacity = population ?? defaults.population;
  return within(residentWorkers, 0, capacity) && within(residentChildren, 0, capacity)
    ? null
    : "residentSlotsInvalid";
}
