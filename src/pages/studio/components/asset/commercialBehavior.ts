export const COMMERCIAL_DEFAULTS = { shoppers: 9, goods: 9 } as const;
export const MIXED_COMMERCIAL = "ground-mall-mixed-v1";
export const COMMERCIAL_SHOPPER_FIELDS = ["shoppersW1", "shoppersW2", "shoppersW3"] as const;
export const COMMERCIAL_FIELDS = [...COMMERCIAL_SHOPPER_FIELDS, "goods"] as const;
export const COMMERCIAL_JOB_FIELDS = ["jobsW1", "jobsW2", "jobsW3", "retailMinWorkers"] as const;
export const COMMERCIAL_PROFILES = {
  "ground-commercial-w1-v1": { shoppers: 10, goods: 21 },
  "ground-commercial-w2-v1": { shoppers: 8, goods: 18 },
  "ground-commercial-w3-v1": { shoppers: 7, goods: 15 },
  "ground-megatower-mall-w2-v1": { shoppers: 180, goods: 750 },
} as const;
export type CommercialProfile = keyof typeof COMMERCIAL_PROFILES | typeof MIXED_COMMERCIAL;

export type CommercialParams = {
  type: "commercial";
  commercialProfile?: CommercialProfile;
  shoppers?: number | null;
} & Partial<Record<(typeof COMMERCIAL_FIELDS | typeof COMMERCIAL_JOB_FIELDS)[number], number | null>>;

export function commercialValues(params: CommercialParams) {
  if (params.commercialProfile === MIXED_COMMERCIAL) {
    return {
      shoppersW1: params.shoppersW1 ?? 60,
      shoppersW2: params.shoppersW2 ?? 60,
      shoppersW3: params.shoppersW3 ?? 60,
      goods: params.goods ?? 750,
    };
  }
  const profile = params.commercialProfile;
  const defaults = profile ? COMMERCIAL_PROFILES[profile] : COMMERCIAL_DEFAULTS;
  if (!defaults) return undefined;
  const capacity = params.shoppers ?? defaults.shoppers;
  const low = profile === "ground-commercial-w1-v1";
  const high = profile === "ground-commercial-w3-v1";
  return {
    shoppersW1: low ? capacity : 0,
    shoppersW2: !low && !high ? capacity : 0,
    shoppersW3: high ? capacity : 0,
    goods: params.goods ?? defaults.goods,
  };
}

export function validCommercial(params: CommercialParams): boolean {
  const values = commercialValues(params);
  if (!values) return false;
  const tiers = COMMERCIAL_SHOPPER_FIELDS.map((field) => values[field]);
  const total = tiers.reduce((sum, value) => sum + value, 0);
  const jobs = COMMERCIAL_JOB_FIELDS.slice(0, 3).reduce((sum, field) => sum + (params[field] ?? 0), 0);
  return tiers.every((value) => Number.isInteger(value) && value >= 0)
    && total >= 1 && total <= 100_000
    && Number.isInteger(values.goods) && values.goods >= 1 && values.goods <= 100_000
    && COMMERCIAL_JOB_FIELDS.every((field) => Number.isInteger(params[field] ?? 0) && (params[field] ?? 0) >= 0)
    && jobs <= 100_000 && (params.retailMinWorkers ?? 0) <= jobs;
}
