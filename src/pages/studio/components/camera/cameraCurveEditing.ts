import { cameraFields, type CameraProfile } from "./cityCamera";

export type CameraField = (typeof cameraFields)[number]["id"];
export type SampleLimits = readonly [number, number];

export function sampleToZoom(
  field: CameraField,
  index: number,
  count: number,
  zoomCount: number,
) {
  if (field === "distance") return Math.min(index, zoomCount - 1);
  return Math.min(
    zoomCount - 1,
    Math.round((index / Math.max(1, count - 1)) * zoomCount),
  );
}

export function zoomToSample(
  field: CameraField,
  zoomIndex: number,
  zoomCount: number,
  count: number,
) {
  if (field === "distance") return Math.min(zoomIndex, count - 1);
  return Math.round((zoomIndex / Math.max(1, zoomCount)) * (count - 1));
}

export function curveSampleLimits(
  profile: CameraProfile,
  field: CameraField,
  index: number,
): SampleLimits {
  const samples = (id: CameraField) =>
    profile.curves.find((curve) =>
      cameraFields.some((item) => item.id === id && item.hash === curve.hash),
    )?.samples;
  const values = samples(field);
  const positive = 2 ** -149;
  if (field === "heading")
    return [-3.4028234663852886e38, 3.4028234663852886e38];
  if (field === "distance")
    return [values?.[index - 1] ?? positive, values?.[index + 1] ?? Infinity];
  if (field === "fov") return [positive, Math.fround(179.99998)];

  const pitch = field === "minPitch" || field === "maxPitch";
  const cap = Math.fround(89.99999);
  let lower = pitch ? -cap : positive;
  let upper = pitch ? cap : Infinity;
  const isLowerCurve = field === "near" || field === "minPitch";
  const counterpart = samples(
    pitch
      ? isLowerCurve
        ? "maxPitch"
        : "minPitch"
      : isLowerCurve
        ? "far"
        : "near",
  );
  const zoomCount = samples("distance")?.length ?? 0;
  if (!counterpart?.length || !values?.length) return [lower, upper];

  // The native controller samples projection/pitch curves at zoomIndex / zoomCount.
  // Solve each interpolated comparison for this point while holding the rest fixed.
  for (let zoom = 0; zoom < zoomCount; zoom++) {
    const ratio = zoom / zoomCount;
    const weight = Math.max(
      0,
      1 - Math.abs((values.length - 1) * ratio - index),
    );
    if (weight === 0) continue;
    const other = interpolate(counterpart, ratio);
    const fixed = interpolate(values, ratio) - values[index]! * weight;
    const gap = pitch ? 0 : Math.max(positive, Math.abs(other) * 1e-7);
    if (isLowerCurve) upper = Math.min(upper, (other - fixed - gap) / weight);
    else lower = Math.max(lower, (other - fixed + gap) / weight);
  }
  // Preserve access to an imported value sitting on the exact game boundary.
  const current = values[index]!;
  return [Math.min(lower, current), Math.max(upper, current)];
}

function interpolate(values: number[], ratio: number) {
  const position = (values.length - 1) * ratio;
  const left = Math.floor(position);
  const start = values[left]!;
  return start + ((values[left + 1] ?? start) - start) * (position - left);
}
