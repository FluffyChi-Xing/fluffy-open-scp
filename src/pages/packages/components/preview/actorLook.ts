import type { ActorInfo } from "@/api/property-semantics";
import type { ActorModelResolution } from "@/api/actor-models";
export interface ActorLook {
  body: number;
  head: number;
  skin: number;
  outfit: number;
  row: number;
}
type Parts = NonNullable<ActorModelResolution["steps"][number]["parts"]>;
/** GetSimPaletteData (0x785AF0): flatten inclusive body/head/outfit combinations. */
export function selectActorLook(
  parts: Parts,
  actor: ActorInfo,
  seed: number,
): ActorLook {
  const ranges = parts.bodies.map((body, row) => {
    const head = parts.heads[row] ?? 0,
      outfit = parts.outfits[row] ?? 0;
    const bodies = Math.max(1, (parts.bodiesMax[row] ?? body) - body + 1);
    const heads = Math.max(1, (parts.headsMax[row] ?? head) - head + 1);
    const outfits = Math.max(1, (parts.outfitsMax[row] ?? outfit) - outfit + 1);
    return {
      row,
      body,
      head,
      outfit,
      bodies,
      heads,
      count: bodies * heads * outfits,
    };
  });
  const count = ranges.reduce((n, r) => n + r.count, 0);
  if (!count || !Number.isSafeInteger(count))
    throw new Error("invalid actor ranges");
  let index = (seed >>> 12) % count;
  let range = ranges[0];
  for (const r of ranges) {
    range = r;
    if (index < r.count) break;
    index -= r.count;
  }
  let body = range.body + (index % range.bodies);
  const head = range.head + (Math.floor(index / range.bodies) % range.heads);
  let outfit = range.outfit + Math.floor(index / range.bodies / range.heads);
  let skin = seed & 255;
  const appearance =
    actor.appearances[(seed >>> 8) % Math.max(1, actor.appearances.length)];
  const pick = (min: number | null, max: number | null, fallback: number) =>
    min != null && min >= 0
      ? min +
        ((seed >>> 16) %
          Math.max(1, (max != null && max >= min ? max : min) - min + 1))
      : fallback;
  if (appearance) {
    body = pick(appearance.body, appearance.body, body);
    outfit = pick(appearance.outfit, appearance.outfitMax, outfit);
    skin = pick(appearance.skin, appearance.skinMax, skin);
  }
  if (
    body < 0 ||
    body > 2 ||
    head < 0 ||
    head > 19 ||
    outfit < 0 ||
    outfit > 255 ||
    skin < 0 ||
    skin > 255
  )
    throw new Error("unsupported actor ranges");
  return { body, head, outfit, skin, row: range.row };
}
