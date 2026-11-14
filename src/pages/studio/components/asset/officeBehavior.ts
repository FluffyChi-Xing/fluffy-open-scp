export const OFFICE_DEFAULTS = { w1: 400, w2: 200, w3: 100, minWorkers: 1 } as const;
export const OFFICE_FIELDS = ["w1", "w2", "w3", "minWorkers"] as const;

export type OfficeParams = {
  type: "office";
} & Partial<Record<(typeof OFFICE_FIELDS)[number], number>>;

export function validOffice(params: OfficeParams): boolean {
  const values = OFFICE_FIELDS.map((field) => params[field] ?? OFFICE_DEFAULTS[field]);
  const [w1, w2, w3, minimum] = values as [number, number, number, number];
  const total = w1 + w2 + w3;
  return values.every((value) => Number.isInteger(value) && value >= 0) &&
    total <= 100_000 && minimum >= 1 && minimum <= total;
}
