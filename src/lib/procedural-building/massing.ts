import type { BlockMassParameters, BuildingPart, Vec3 } from "./schema";

export const MASS_LIMITS = {
  blocks: 16,
  voids: 32,
  size: 1000,
  position: 2000,
} as const;
const finite = (n: unknown, fallback: number) =>
  typeof n === "number" && Number.isFinite(n) ? n : fallback;
const clamp = (n: unknown, fallback: number, min: number, max: number) =>
  Math.max(min, Math.min(max, finite(n, fallback)));
const vector = (
  v: Vec3 | undefined,
  fallback: Vec3,
  min: number,
  max: number,
): Vec3 => fallback.map((n, i) => clamp(v?.[i], n, min, max)) as Vec3;

export function normalizeBlockMass(
  input: BlockMassParameters,
): BlockMassParameters {
  const ids = new Set<string>();
  const blocks = input.blocks.slice(0, MASS_LIMITS.blocks).map((block, i) => {
    let id = block.id || `block-${i + 1}`;
    while (ids.has(id)) id += "-copy";
    ids.add(id);
    const size = vector(block.size, [48, 24, 120], 1, MASS_LIMITS.size);
    const center = vector(
      block.center,
      [0, 0, size[2] / 2],
      -MASS_LIMITS.position,
      MASS_LIMITS.position,
    );
    center[2] = Math.max(size[2] / 2, center[2]);
    return {
      id,
      size,
      center,
      ...(block.junction
        ? {
            junction: [
              clamp(block.junction[0], 0, -2000, 2000),
              clamp(block.junction[1], 0, -2000, 2000),
            ] as [number, number],
          }
        : {}),
      rotation: ((finite(block.rotation, 0) % 360) + 360) % 360,
    };
  });
  const holeIds = new Set<string>();
  const voids = input.voids
    .filter((h) => ids.has(h.blockId))
    .slice(0, MASS_LIMITS.voids)
    .map((hole, i) => {
      let id = hole.id || `void-${i + 1}`;
      while (holeIds.has(id)) id += "-copy";
      holeIds.add(id);
      return {
        id,
        blockId: hole.blockId,
        center: vector(hole.center, [0, 0, 0], -2000, 2000),
        size: vector(hole.size, [8, 8, 8], 0.1, 2000),
      };
    });
  return { blocks, voids };
}

export function generateMassParts(input: BlockMassParameters): BuildingPart[] {
  const mass = normalizeBlockMass(input);
  return mass.blocks.map((block) => ({
    id: block.id,
    kind: "mass",
    slot: "wall",
    center: block.center,
    size: block.size,
    rotation: (block.rotation * Math.PI) / 180,
    cutouts: mass.voids
      .filter((h) => h.blockId === block.id)
      .map((h) => ({ center: h.center, size: h.size })),
  }));
}

export type MassPreset = "single" | "y" | "courtyard";
/** Each wing starts at the shared junction; changing its angle preserves attachment. */
export function massPreset(preset: MassPreset): BlockMassParameters {
  if (preset === "single")
    return {
      blocks: [
        { id: "block-1", size: [72, 54, 120], center: [0, 0, 60], rotation: 0 },
      ],
      voids: [],
    };
  if (preset === "courtyard")
    return {
      blocks: [
        {
          id: "block-1",
          size: [140, 110, 150],
          center: [0, 0, 75],
          rotation: 0,
        },
      ],
      voids: [
        {
          id: "void-1",
          blockId: "block-1",
          size: [80, 50, 152],
          center: [0, 0, 0],
        },
      ],
    };
  return {
    blocks: [0, 120, 240].map((rotation, i) => ({
      id: `block-${i + 1}`,
      size: [84, 28, 180] as Vec3,
      center: [
        32 * Math.cos((rotation * Math.PI) / 180),
        32 * Math.sin((rotation * Math.PI) / 180),
        90,
      ] as Vec3,
      rotation,
      junction: [0, 0] as [number, number],
    })),
    voids: [],
  };
}

export function rotateBlock(
  block: import("./schema").BuildingBlock,
  rotation: number,
): import("./schema").BuildingBlock {
  if (!block.junction) return { ...block, rotation };
  const [jx, jy] = block.junction,
    delta = ((rotation - block.rotation) * Math.PI) / 180;
  const x = block.center[0] - jx,
    y = block.center[1] - jy;
  return {
    ...block,
    rotation,
    center: [
      jx + x * Math.cos(delta) - y * Math.sin(delta),
      jy + x * Math.sin(delta) + y * Math.cos(delta),
      block.center[2],
    ],
  };
}
