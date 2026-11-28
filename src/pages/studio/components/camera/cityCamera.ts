import { focusFields } from "./cityCameraFocus";

export const cameraFields = [
  { id: "distance", hash: 0x00fc5228, unit: "m" },
  { id: "fov", hash: 0x00fc6857, unit: "°" },
  { id: "near", hash: 0x00fc7047, unit: "m" },
  { id: "far", hash: 0x00fc704c, unit: "m" },
  { id: "minPitch", hash: 0x00fc71fc, unit: "°" },
  { id: "maxPitch", hash: 0x00fc7205, unit: "°" },
  { id: "heading", hash: 0x00fc78e7, unit: "°" },
] as const;

export interface CameraCurve {
  hash: number;
  samples: number[];
  inherited: boolean;
}

export interface CameraProfile {
  instance: number;
  curves: CameraCurve[];
  autoFocus?: number[][] | null;
}

export interface CameraSource {
  revision: string;
  profiles: CameraProfile[];
  motionStrength: number;
}

export interface CameraFrame {
  eye: [number, number, number];
  target: [number, number, number];
  fov: number;
  near: number;
  far: number;
  pitch: number;
  distance: number;
  pitchLocked: boolean;
}

export function hex(value: number) {
  return value.toString(16).toUpperCase().padStart(8, "0");
}

export function curveKey(instance: number, hash: number) {
  return `${hex(instance)}:${hex(hash)}`;
}

export function parseSamples(hash: number, input: string) {
  const tokens = input.trim().split(/[\s,]+/);
  if (
    tokens.length > 1024 ||
    tokens.some(
      (token) => !/^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i.test(token),
    )
  )
    return null;
  const samples = tokens.map((token) => Math.fround(Number(token)));
  const focus = focusFields.find((item) => item.hash === hash);
  if (focus) {
    const value = samples[0]!;
    const valid =
      focus.id === "mode"
        ? [0, 1, 2].includes(value)
        : Number.isFinite(value) &&
          (focus.id === "radius" ? value >= 0 : value > 0);
    return samples.length === 1 && valid ? samples : null;
  }
  const field = cameraFields.find((item) => item.hash === hash)?.id;
  const valid = samples.every((value, i) => {
    if (!Number.isFinite(value)) return false;
    switch (field) {
      case "distance":
        return value > 0 && (i === 0 || value >= samples[i - 1]!);
      case "fov":
        return value > 0 && value < 180;
      case "minPitch":
      case "maxPitch":
        return value > -90 && value < 90;
      case "near":
      case "far":
        return value > 0;
      case "heading":
        return true;
      default:
        return false;
    }
  });
  return valid ? samples : null;
}

function lookup(samples: number[], fraction: number) {
  const position = (samples.length - 1) * fraction;
  const i = Math.floor(position);
  const value = samples[i]!;
  return value + ((samples[i + 1] ?? value) - value) * (position - i);
}

export function cameraFrame(
  profile: CameraProfile,
  zoomIndex: number,
  headingIndex: number,
  tilt: number,
): CameraFrame | null {
  const [zoom, fovCurve, nearCurve, farCurve, minPitch, maxPitch, headings] =
    cameraFields.map(
      ({ hash }) =>
        profile.curves.find((curve) => curve.hash === hash)?.samples,
    );
  if (
    !zoom?.length ||
    !fovCurve?.length ||
    !nearCurve?.length ||
    !farCurve?.length ||
    !minPitch?.length ||
    !maxPitch?.length ||
    !headings?.length
  )
    return null;

  // Dev controller SetZoomLevel / UpdateZoomLevel / GetTargetPitch use index / count.
  const index = Math.min(zoomIndex, zoom.length - 1);
  const fraction = index / zoom.length;
  const distance = zoom[index]!;
  const lowPitch = lookup(minPitch, fraction);
  const highPitch = lookup(maxPitch, fraction);
  const pitch = lowPitch + (highPitch - lowPitch) * tilt;
  const heading = headings[Math.min(headingIndex, headings.length - 1)]!;
  const fov = lookup(fovCurve, fraction);
  const near = lookup(nearCurve, fraction);
  const far = lookup(farCurve, fraction);
  if (
    distance <= 0 ||
    lowPitch > highPitch ||
    pitch <= -90 ||
    pitch >= 90 ||
    fov <= 0 ||
    fov >= 180 ||
    near <= 0 ||
    far <= near
  )
    return null;

  // FindTransform uses Z-up. The preview maps game (x, y, z) to Three (x, z, -y).
  const rise = Math.tan((pitch * Math.PI) / 180);
  const run = distance / Math.sqrt(1 + rise * rise);
  const eyeHeight = Math.max(1.7, run * rise);
  const azimuth = (heading * Math.PI) / 180;
  return {
    eye: [run * Math.cos(azimuth), eyeHeight, -run * Math.sin(azimuth)],
    target: [0, eyeHeight - run * rise, 0],
    fov,
    near,
    far,
    pitch,
    distance,
    pitchLocked: lowPitch === highPitch,
  };
}
