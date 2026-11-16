import type { CameraProfile } from "./cityCamera";

export const focusFields = [
  { id: "mode", hash: 0x0d2d0452 },
  { id: "distance", hash: 0x0e9ffe89 },
  { id: "radius", hash: 0x0e9ffe8a },
  { id: "nearRamp", hash: 0x0e9ffe8b },
  { id: "farRamp", hash: 0x0e9ffe8c },
] as const;

export interface FocusPlane {
  distance: number;
  radius: number;
  nearRamp: number;
  farRamp: number;
}

function halton(index: number, base: number) {
  let value = 0;
  let factor = 1;
  while (index > 0) {
    factor /= base;
    value += factor * (index % base);
    index = Math.floor(index / base);
  }
  return value;
}

// Update samples twelve points around the viewport center. The workbench
// intersects the rays with its flat ground and uses the settled focus values.
export function groundFocusDistance(
  height: number,
  pitch: number,
  verticalFov: number,
  aspect: number,
  area: number,
) {
  const vertical = Math.tan((verticalFov * Math.PI) / 360);
  const angle = (pitch * Math.PI) / 180;
  let sum = 0;
  let hits = 0;
  for (let i = 0; i < 12; i++) {
    const u = (2 * halton(i, 2) - 1) * area * vertical * aspect;
    const v = (1 - 2 * halton(i, 3)) * area * vertical;
    const down = Math.sin(angle) - v * Math.cos(angle);
    if (down > 0) {
      sum += (height / down) * Math.hypot(1, u, v);
      hits++;
    }
  }
  return hits ? sum / hits : null;
}

export function focusPlane(
  profile: CameraProfile | undefined,
  height: number,
  pitch: number,
  fov: number,
  aspect: number,
): FocusPlane | null {
  const [mode, distance, radius, nearRamp, farRamp] = focusFields.map(
    ({ hash }) =>
      profile?.curves.find((curve) => curve.hash === hash)?.samples[0],
  );
  if (
    mode === 2 &&
    distance !== undefined &&
    radius !== undefined &&
    nearRamp !== undefined &&
    farRamp !== undefined &&
    nearRamp > 0 &&
    farRamp > 0
  ) {
    return { distance, radius, nearRamp, farRamp };
  }
  const table = profile?.autoFocus;
  const row = table?.[1] ?? table?.[0];
  if (mode !== 1 || !row || row.length !== 11) return null;
  const [
    offset,
    minimum,
    maximum,
    radiusScalar,
    area,
    nearOut,
    nearIn,
    nearRatio,
    farRatio,
  ] = row as [
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
  ];
  const sample = groundFocusDistance(height, pitch, fov, aspect, area);
  const ground = Math.min(sample ?? 1024, maximum * 2);
  const clamp = (value: number) => Math.min(maximum, Math.max(minimum, value));
  const autoRadius = clamp(ground * radiusScalar);
  if (nearRatio < 0) {
    const adjusted = clamp(
      (Math.min(autoRadius * 2 + nearIn, maximum * 2) - nearIn) * 0.5,
    );
    return {
      distance: adjusted + nearIn,
      radius: adjusted,
      nearRamp: nearIn - nearOut,
      farRamp: adjusted * farRatio,
    };
  }
  return {
    distance: (1 + offset) * ground,
    radius: autoRadius,
    nearRamp: nearRatio * autoRadius,
    farRamp: farRatio * autoRadius,
  };
}
