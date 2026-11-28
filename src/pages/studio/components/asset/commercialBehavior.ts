export const MIXED_COMMERCIAL = "ground-mall-mixed-v1";
const W2_MALL = "ground-megatower-mall-w2-v1";
export const COMMERCIAL_SHOPPER_FIELDS = ["shoppersW1", "shoppersW2", "shoppersW3"] as const;
export const COMMERCIAL_GOODS_FIELDS = ["goodsW1", "goodsW2", "goodsW3"] as const;
export const COMMERCIAL_FIELDS = [...COMMERCIAL_SHOPPER_FIELDS, ...COMMERCIAL_GOODS_FIELDS] as const;
export const COMMERCIAL_JOB_FIELDS = ["jobsW1", "jobsW2", "jobsW3", "retailMinWorkers"] as const;
export const COMMERCIAL_PROFILES = {
  "ground-commercial-w1-v1": { shoppers: 10, goods: 21 },
  "ground-commercial-w2-v1": { shoppers: 8, goods: 18 },
  "ground-commercial-w3-v1": { shoppers: 7, goods: 15 },
  [W2_MALL]: { shoppers: 180, goods: 750 },
} as const;
export type CommercialProfile = keyof typeof COMMERCIAL_PROFILES | typeof MIXED_COMMERCIAL;

export type CommercialParams = {
  type: "commercial";
  commercialProfile?: CommercialProfile;
  shoppers?: number | null;
  goods?: number | null;
} & Partial<Record<(typeof COMMERCIAL_FIELDS | typeof COMMERCIAL_JOB_FIELDS)[number], number | null>>;

function commercialProfile(params: CommercialParams): CommercialProfile {
  const hasLegacyQuota = params.shoppers !== null && params.shoppers !== undefined;
  const hasTierQuota = COMMERCIAL_SHOPPER_FIELDS.some((field) => params[field] !== null && params[field] !== undefined);
  return params.commercialProfile ?? (hasLegacyQuota && !hasTierQuota ? W2_MALL : MIXED_COMMERCIAL);
}

export function usesMallProfile(params: CommercialParams): boolean {
  const profile = commercialProfile(params);
  return profile === MIXED_COMMERCIAL || profile === W2_MALL;
}

function goodsValues(params: CommercialParams, fallback: number) {
  const medium = params.goods ?? fallback;
  return {
    goodsW1: params.goodsW1 ?? Math.min(100_000, medium * 2),
    goodsW2: params.goodsW2 ?? medium,
    goodsW3: params.goodsW3 ?? Math.max(1, Math.floor(medium / 2)),
  };
}

export function commercialValues(params: CommercialParams) {
  const profile = commercialProfile(params);
  if (profile === MIXED_COMMERCIAL) {
    return {
      shoppersW1: params.shoppersW1 ?? 60,
      shoppersW2: params.shoppersW2 ?? 60,
      shoppersW3: params.shoppersW3 ?? 60,
      ...goodsValues(params, 750),
    };
  }
  const defaults = COMMERCIAL_PROFILES[profile];
  if (!defaults) return undefined;
  const capacity = params.shoppers ?? defaults.shoppers;
  const low = profile === "ground-commercial-w1-v1";
  const high = profile === "ground-commercial-w3-v1";
  return {
    shoppersW1: low ? capacity : 0,
    shoppersW2: !low && !high ? capacity : 0,
    shoppersW3: high ? capacity : 0,
    ...goodsValues(params, defaults.goods),
  };
}

export function validCommercial(params: CommercialParams): boolean {
  const values = commercialValues(params);
  if (!values) return false;
  const tiers = COMMERCIAL_SHOPPER_FIELDS.map((field) => values[field]);
  const total = tiers.reduce((sum, value) => sum + value, 0);
  return tiers.every((value) => Number.isInteger(value) && value >= 0)
    && total >= 1 && total <= 100_000
    && COMMERCIAL_GOODS_FIELDS.every((field) => Number.isInteger(values[field])
      && values[field] >= 1 && values[field] <= 100_000);
}
