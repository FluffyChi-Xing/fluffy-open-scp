import * as THREE from "three";
import type { Region3DData } from "@/lib/region-map";
import { paintStateResourceTile, stateResourceTiles, type ResourceTile } from "./map-state-fields";

const overlaps = (a: ResourceTile, b: ResourceTile) =>
  a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;

export function createEditedResourceTexture(kind: string, size: number) {
  const pixels = new Uint8Array(size * size * 4);
  const texture = new THREE.DataTexture(pixels, size, size, THREE.RGBAFormat);
  texture.minFilter = texture.magFilter = THREE.NearestFilter;
  let previous: ResourceTile[] = [];
  let sampling: number[] = [];
  let partial = false;
  let uploaded = false;
  texture.onUpdate = () => { uploaded = true; };
  return {
    texture,
    update(data: Region3DData) {
      const tiles = stateResourceTiles(data, kind, size);
      const nextSampling = [...data.originWorld, data.metersPerPixel];
      const sameLayout = sampling.length === nextSampling.length
        && sampling.every((value, i) => value === nextSampling[i])
        && previous.length === tiles.length
        && tiles.every((tile, i) => tile.site === previous[i]!.site
          && tile.bounds.every((value, axis) => value === previous[i]!.bounds[axis]));
      const changed = tiles.filter((tile, i) => tile.map !== previous[i]?.map);
      if (sameLayout && !changed.length) return;
      if (sameLayout && partial) {
        const range = uploaded ? (start: number, count: number) => texture.addUpdateRange(start, count) : undefined;
        for (const tile of changed) {
          paintStateResourceTile(pixels, size, data, tile, range);
        }
        if (!uploaded || texture.updateRanges.length) texture.needsUpdate = true;
      } else {
        pixels.fill(0);
        for (const tile of tiles) paintStateResourceTile(pixels, size, data, tile);
        texture.clearUpdateRanges();
        texture.needsUpdate = true;
        partial = !tiles.some((tile, i) => tiles.slice(i + 1).some((other) => overlaps(tile, other)));
      }
      previous = tiles;
      sampling = nextSampling;
    },
  };
}
