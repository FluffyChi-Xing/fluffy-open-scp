import type { RoadAppearance } from "@/pages/packages/components/preview/roadAppearanceScene";
import type { RoadRibbon } from "@/lib/region-map";
import type { RoadStrip } from "./roadDraft";
import { recipes, type PavedRecipe } from "./roadSchema";

const DIVIDER_BANK = 1;
const TRACK_BANK = 2;
const TREE_BANK = 3;

function slice(ribbon: RoadRibbon, first: number, last: number): RoadRibbon {
  const reversed = ribbon.scale[0] < 0;
  const start = reversed ? ribbon.uvEnd[0] : ribbon.uvStart[0];
  const end = reversed ? ribbon.uvStart[0] : ribbon.uvEnd[0];
  return {
    ...ribbon,
    uvStart: [start + (end - start) * first, ribbon.uvStart[1]],
    uvEnd: [start + (end - start) * last, ribbon.uvEnd[1]],
  };
}

function fit(ribbon: RoadRibbon, center: number, width: number): RoadRibbon {
  return {
    ...ribbon,
    offset: [center, ribbon.offset[1], ribbon.offset[2]],
    worldSize: [width, ribbon.worldSize[1]],
    scale: [1, ribbon.scale[1], ribbon.scale[2]],
  };
}

export function composeRoadDraftAppearance(
  instance: number,
  assets: RoadAppearance,
  recipe: PavedRecipe,
  strips: readonly RoadStrip[],
): RoadAppearance {
  const template = recipes[recipe];
  const flat = (assets.ribbons[instance] ?? []).filter(
    (ribbon) =>
      ribbon.offset[2] >= 0 && !ribbon.rotation?.[1] && !ribbon.rotation?.[2],
  );
  const nearest = (x: number) =>
    flat.reduce<RoadRibbon | undefined>(
      (best, ribbon) =>
        !best || Math.abs(ribbon.offset[0] - x) < Math.abs(best.offset[0] - x)
          ? ribbon
          : best,
      undefined,
    );
  const selectedMedian =
    recipe === "mediumAvenue" || recipe === "highAvenue"
      ? nearest(0)
      : undefined;
  const median = selectedMedian ?? assets.ribbons[DIVIDER_BANK]?.[0];
  const ribbons: RoadRibbon[] = [];
  const sweeps: RoadAppearance["sweeps"][string] = [];
  const unsupportedComponents: string[] = [];
  let left = -strips.reduce((sum, strip) => sum + strip.width, 0) / 2;

  for (const strip of strips) {
    const center = left + strip.width / 2;
    left += strip.width;
    const side = center < 0 ? -1 : 1;
    if (strip.kind === "rail") {
      const track = assets.ribbons[TRACK_BANK] ?? [];
      const models = assets.sweeps[TRACK_BANK] ?? [];
      const base = track[0];
      if (!base || (track.length < 2 && !models.length)) {
        unsupportedComponents.push(strip.id);
        continue;
      }
      const factor = strip.width / Math.abs(base.worldSize[0] * base.scale[0]);
      ribbons.push(
        ...track.map((ribbon) => ({
          ...ribbon,
          offset: [
            center + ribbon.offset[0] * factor,
            ribbon.offset[1],
            ribbon.offset[2],
          ] as RoadRibbon["offset"],
          scale: [
            ribbon.scale[0] * factor,
            ribbon.scale[1],
            ribbon.scale[2],
          ] as RoadRibbon["scale"],
        })),
      );
      sweeps.push(
        ...models.map((sweep) => ({
          ...sweep,
          offset: [
            center + sweep.offset[0] * factor,
            sweep.offset[1],
            sweep.offset[2],
          ] as RoadRibbon["offset"],
          scale: [
            sweep.scale[0] * factor,
            sweep.scale[1],
            sweep.scale[2],
          ] as RoadRibbon["scale"],
        })),
      );
      continue;
    }

    let material: RoadRibbon | undefined = median && slice(median, 0, 1);
    if (strip.kind === "walk") {
      const source = nearest(side * template.walkOffset);
      material = undefined;
      if (source) {
        const fraction = Math.min(
          1,
          template.walkWidth / Math.abs(source.worldSize[0] * source.scale[0]),
        );
        material =
          side < 0
            ? slice(source, 0, fraction)
            : slice(source, 1 - fraction, 1);
      }
    } else if (strip.kind === "forward" || strip.kind === "backward") {
      const travelSide = strip.kind === "forward" ? 1 : -1;
      const source = nearest(
        travelSide * (template.offset + (template.lanes * template.width) / 2),
      );
      material = undefined;
      if (source) {
        const fraction = Math.min(
          1,
          1 /
            Math.max(
              1,
              Math.round(
                Math.abs(source.worldSize[0] * source.scale[0]) /
                  template.width,
              ),
            ),
        );
        material =
          travelSide < 0
            ? slice(source, 0, fraction)
            : slice(source, 1 - fraction, 1);
      }
    }
    if (material) ribbons.push(fit(material, center, strip.width));
    else unsupportedComponents.push(strip.id);
    if (strip.kind === "trees") {
      const trees = assets.sweeps[TREE_BANK] ?? [];
      if (!trees.length) unsupportedComponents.push(strip.id);
      sweeps.push(
        ...trees.map((tree) => ({
          ...tree,
          offset: [center, tree.offset[1], 0] as RoadRibbon["offset"],
        })),
      );
    }
  }

  const textureIds = new Set(ribbons.map(({ texture }) => String(texture)));
  const modelIds = new Set(sweeps.map(({ model }) => String(model)));
  return {
    instances: [],
    ribbons: { [instance]: ribbons },
    sweeps: { [instance]: sweeps },
    textures: Object.fromEntries(
      Object.entries(assets.textures).filter(([id]) => textureIds.has(id)),
    ),
    models: Object.fromEntries(
      Object.entries(assets.models).filter(([id]) => modelIds.has(id)),
    ),
    unsupportedComponents,
  };
}
