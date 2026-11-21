import type { ActorLook } from "./actorLook";
export interface Pixels {
  width: number;
  height: number;
  rgba: number[];
}
export interface ActorPart {
  glbs: number[][];
  source: Pixels | null;
  headJoint: number | null;
  animationCount: number;
}
export interface ActorVisualPayload {
  body: ActorPart;
  head: ActorPart | null;
  bodyPalette: Pixels;
  clothesPalette: Pixels;
}
/** Native shader sampling at palette column centers; linear filtering along V. */
export function bakeActorTexture(
  source: Pixels,
  body: Pixels,
  clothes: Pixels,
  look: Pick<ActorLook, "skin" | "outfit">,
): Uint8Array<ArrayBuffer> {
  if (
    body.width !== 256 ||
    clothes.width !== 256 ||
    body.height !== 32 ||
    clothes.height !== 32 ||
    source.rgba.length !== source.width * source.height * 4
  )
    throw new Error("invalid palette dimensions");
  const sample = (p: Pixels, x: number, v: number, c: number) => {
    const y = Math.max(0, Math.min(p.height - 1, v * p.height - 0.5));
    const lo = Math.floor(y),
      hi = Math.min(p.height - 1, lo + 1),
      f = y - lo;
    return (
      ((p.rgba[(lo * p.width + x) * 4 + c] ?? 0) * (1 - f) +
        (p.rgba[(hi * p.width + x) * 4 + c] ?? 0) * f) /
      255
    );
  };
  const rgba = new Uint8Array(source.rgba.length);
  for (let i = 0; i < rgba.length; i += 4) {
    const ao = source.rgba[i] / 255,
      v = source.rgba[i + 1] / 255,
      mask = sample(clothes, look.outfit, v, 3);
    for (let c = 0; c < 3; c++)
      rgba[i + c] = Math.round(
        255 *
          ao *
          (sample(body, look.skin, v, c) * (1 - mask) +
            sample(clothes, look.outfit, v, c) * mask),
      );
    rgba[i + 3] = source.rgba[i + 3];
  }
  return rgba;
}
