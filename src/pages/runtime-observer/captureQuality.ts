import type { Raw } from "./diagnostics";
const obj = (v: unknown): Raw =>
  v !== null && typeof v === "object" && !Array.isArray(v) ? (v as Raw) : {};
const count = (v: unknown) =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0 ? v : undefined;
export interface CaptureQuality {
  reported?: number;
  dropped?: number;
  decodeErrors?: number;
  errors?: number;
  hostErrors?: number;
  incompleteRequests?: number;
  detached?: boolean;
  countMatches?: boolean;
  channels: {
    name: string;
    hits?: number;
    accepted?: number;
    dropped?: number;
    filtered?: number;
    queued?: number;
  }[];
}
/** Unknown counters stay unknown. Channels overlap top-level dropped; never add both. */
export function captureQuality(
  status: Raw,
  recordCount: number,
): CaptureQuality {
  const summary = obj(status.summary);
  const reported = count(status.records) ?? count(summary.total);
  const channels = Object.entries(obj(status.channels)).map(([name, value]) => {
    const c = obj(value);
    return {
      name,
      hits: count(c.hits),
      accepted: count(c.accepted),
      dropped: count(c.dropped),
      filtered: count(c.filtered),
      queued: count(c.queued),
    };
  });
  const agentErrors = Array.isArray(status.agentErrors)
    ? status.agentErrors.length
    : undefined;
  const errors = count(summary.errors) ?? agentErrors;
  return {
    reported,
    dropped: count(status.dropped) ?? count(summary.dropped),
    decodeErrors: count(status.decodeErrors),
    errors,
    incompleteRequests: count(summary.incompleteRequests),
    hostErrors: Array.isArray(status.errors) ? status.errors.length : undefined,
    detached:
      typeof status.detached === "boolean" ? status.detached : undefined,
    countMatches: reported === undefined ? undefined : reported === recordCount,
    channels,
  };
}
