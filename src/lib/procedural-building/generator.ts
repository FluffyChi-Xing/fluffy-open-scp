import {
  type BalconyMode,
  type BuildingPart,
  type DetailStyle,
  type MaterialSlotId,
  type PartKind,
  type ProceduralBuildingParameters,
  buildingFootprint,
} from "./schema";
import { mulberry32 } from "./rng";
import { normalizeBlockMass, generateMassParts } from "./massing";

/** 立面模块算法（参考 ProceduralBuildingsThreeJS 的 generator.ts 移植）：
 * 1 开间 = 3 m 的立面节奏；底层挑高；标准层逐开间开窗位；腰线层
 * （第 1 层与顶层）出挑阳台；檐口 + 孟莎/平顶；墙面细节（壁柱/横带）
 * 按风格或 seed 逐层变化。
 *
 * 【渲染口径】窗户与大门不建模——游戏里窗户由立面贴图（wall 槽绑定
 * 带窗贴图）表达，建模反而是多余面片；只有阳台、老虎窗这类出挑
 * 体量保留几何。
 *
 * 全部部件为轴对齐盒体（Z-up，原点 = 地块中心/地面），由 meshBuilder
 * 细分拼装成网格。算法本身零依赖（不感知 three.js 与游戏资源）。
 */

const BAY = 3;
const WALL_T = 0.3;

/** 盒体部件工厂：id/kind/slot + 中心 + 尺寸。 */
function part(
  id: string,
  kind: PartKind,
  slot: MaterialSlotId,
  center: [number, number, number],
  size: [number, number, number],
): BuildingPart {
  return { id, kind, slot, center, size };
}

/** 立面的一侧：沿边方向的开间数与墙面外法线方向。 */
interface SidePlan {
  key: "front" | "back" | "left" | "right";
  bays: number;
  /** 墙面外法线单位向量（轴对齐）。 */
  normal: [number, number];
  /** 沿边方向的单位向量。 */
  along: [number, number];
}

function sidePlans(baysX: number, baysY: number): SidePlan[] {
  return [
    { key: "front", bays: baysX, normal: [0, 1], along: [1, 0] },
    { key: "back", bays: baysX, normal: [0, -1], along: [1, 0] },
    { key: "left", bays: baysY, normal: [-1, 0], along: [0, 1] },
    { key: "right", bays: baysY, normal: [1, 0], along: [0, 1] },
  ];
}

/** 侧边局部坐标 → 世界坐标：法线方向偏移 outOffset，沿边方向偏移 u。 */
function sidePoint(
  side: SidePlan,
  faceOffset: number,
  u: number,
  outOffset: number,
): [number, number] {
  const cx = side.normal[0] * (faceOffset + outOffset) + side.along[0] * u;
  const cy = side.normal[1] * (faceOffset + outOffset) + side.along[1] * u;
  return [cx, cy];
}

function levelSpan(
  parameters: ProceduralBuildingParameters,
  level: number,
): {
  base: number;
  height: number;
} {
  if (level === 0) return { base: 0, height: parameters.groundFloorHeight };
  return {
    base: parameters.groundFloorHeight + (level - 1) * parameters.floorHeight,
    height: parameters.floorHeight,
  };
}

/** 阳台层（featured = 第 1 标准层 + 顶层）。 */
function balconyRows(mode: BalconyMode, floors: number): Set<number> {
  if (mode === "none" || floors < 1) return new Set();
  if (mode === "all")
    return new Set(Array.from({ length: floors }, (_, i) => i + 1));
  return new Set([1, floors]);
}

/** 每层墙面细节的确定性选择（random 风格用 seed+层号散列）。 */
function levelDetail(
  style: DetailStyle,
  level: number,
  seed: number,
): "pilasters" | "refend" | "none" {
  if (style === "uniform") return "pilasters";
  if (style === "alternating") return level % 2 === 1 ? "refend" : "pilasters";
  if (style === "random") {
    const roll = mulberry32((seed ^ (level * 0x9e3779b9)) >>> 0)();
    if (roll < 0.4) return "pilasters";
    if (roll < 0.7) return "refend";
    return "none";
  }
  return "none";
}

/** 参数钳制：滑杆/导入文档可能携带越界值，生成器自兜底。 */
export function normalizeParameters(
  input: Partial<ProceduralBuildingParameters>,
): ProceduralBuildingParameters {
  const clamp = (value: number, min: number, max: number) =>
    Math.max(min, Math.min(max, value));
  return {
    baysX: clamp(Math.round(input.baysX ?? 6), 1, 12),
    baysY: clamp(Math.round(input.baysY ?? 4), 1, 12),
    floors: clamp(Math.round(input.floors ?? 5), 1, 40),
    groundFloorHeight: clamp(Number(input.groundFloorHeight ?? 4.2), 3, 8),
    floorHeight: clamp(Number(input.floorHeight ?? 3.2), 2.4, 8),
    roofStyle: input.roofStyle === "flat" ? "flat" : "mansard",
    balconies:
      input.balconies === "none" || input.balconies === "all"
        ? input.balconies
        : "featured",
    detail:
      input.detail === "off" ||
      input.detail === "uniform" ||
      input.detail === "random"
        ? input.detail
        : "alternating",
    ...(input.windowSpacing === undefined
      ? {}
      : {
          windowSpacing: Number.isFinite(Number(input.windowSpacing))
            ? Math.max(0, Math.min(12, Number(input.windowSpacing)))
            : 0,
        }),
    ...(input.blockMass
      ? { blockMass: normalizeBlockMass(input.blockMass) }
      : {}),
  };
}

export function generateBuildingParts(
  rawParameters: Partial<ProceduralBuildingParameters>,
  seed: number,
): BuildingPart[] {
  const parameters = normalizeParameters(rawParameters);
  if (parameters.blockMass) return generateMassParts(parameters.blockMass);
  const { width, depth } = buildingFootprint(parameters);
  const W = width;
  const D = depth;
  const hw = W / 2;
  const hd = D / 2;
  const parts: BuildingPart[] = [];
  const totalLevels = parameters.floors + 1;
  const top =
    parameters.groundFloorHeight + parameters.floors * parameters.floorHeight;
  const rng = mulberry32(seed);

  // 基础与室内地坪
  parts.push(
    part(
      "foundation",
      "foundation",
      "ground",
      [0, 0, -0.25],
      [W + 0.6, D + 0.6, 0.5],
    ),
  );
  parts.push(part("slab", "slab", "ground", [0, 0, 0.06], [W, D, 0.12]));

  // 转角壁柱（每层一段，角点外突）
  for (let level = 0; level < totalLevels; level += 1) {
    const { base, height } = levelSpan(parameters, level);
    const zc = base + height / 2;
    for (const sx of [-1, 1]) {
      for (const sy of [-1, 1]) {
        parts.push(
          part(
            `pier-${level}-${sx > 0 ? "r" : "l"}${sy > 0 ? "f" : "b"}`,
            "pier",
            "trim",
            [sx * hw, sy * hd, zc],
            [0.8, 0.8, height],
          ),
        );
      }
    }
  }

  const sides = sidePlans(parameters.baysX, parameters.baysY);

  // 墙板：每侧一块全高（立面贴图整幅一次的映射单元；窗户由 wall 槽
  // 立面贴图表达，不建模）
  for (const side of sides) {
    const isXWall = side.along[0] !== 0;
    const wallHalf = (isXWall ? W : D) / 2;
    const wallFace = isXWall ? hd : hw;
    const [wx, wy] = sidePoint(side, wallFace - WALL_T / 2, 0, 0);
    const wallSpan = wallHalf * 2 - (isXWall ? 2 * WALL_T : 0);
    parts.push(
      part(
        `wall-${side.key}`,
        "wall",
        "wall",
        [wx, wy, top / 2],
        isXWall ? [wallSpan, WALL_T, top] : [WALL_T, wallSpan, top],
      ),
    );
  }

  for (const side of sides) {
    const isX = side.along[0] !== 0;
    const halfSpan = (isX ? W : D) / 2;
    // 墙面到中心的距离 = 法线轴的半跨：front/back 法线为 y（hd），left/right 法线为 x（hw）
    const faceOffset = isX ? hd : hw;

    for (let level = 0; level < totalLevels; level += 1) {
      const { base, height } = levelSpan(parameters, level);
      const zc = base + height / 2;

      if (level === 0) {
        // 底层顶部的过梁带（骑楼贴图分界的收边）
        const lintelH = Math.min(0.5, parameters.groundFloorHeight * 0.15);
        if (lintelH > 0.15) {
          const [lx, ly] = sidePoint(side, faceOffset, 0, 0.03);
          parts.push(
            part(
              `lintel-${side.key}`,
              "refend",
              "trim",
              [lx, ly, parameters.groundFloorHeight - lintelH / 2],
              isX
                ? [halfSpan * 2, 0.12, lintelH]
                : [0.12, halfSpan * 2, lintelH],
            ),
          );
        }
        continue;
      }

      // 阳台：板 + 栏杆（出挑体量，保留几何）
      if (balconyRows(parameters.balconies, parameters.floors).has(level)) {
        for (let bay = 0; bay < side.bays; bay += 1) {
          const u = (bay - (side.bays - 1) / 2) * BAY;
          const [sx, sy] = sidePoint(side, faceOffset, u, 0.62);
          const [rx, ry] = sidePoint(side, faceOffset, u, 1.18);
          parts.push(
            part(
              `balcony-${side.key}-${level}-${bay + 1}`,
              "balconySlab",
              "trim",
              [sx, sy, base - 0.09],
              isX ? [2.7, 1.24, 0.18] : [1.24, 2.7, 0.18],
            ),
          );
          parts.push(
            part(
              `balrail-${side.key}-${level}-${bay + 1}`,
              "balconyRail",
              "trim",
              [rx, ry, base + 0.42],
              isX ? [2.5, 0.08, 1.0] : [0.08, 2.5, 1.0],
            ),
          );
        }
      }

      // 墙面细节（逐层风格）
      const detail = levelDetail(parameters.detail, level, seed);
      if (detail === "pilasters") {
        for (let bay = 1; bay < side.bays; bay += 1) {
          const u = (bay - side.bays / 2) * BAY;
          const [px, py] = sidePoint(side, faceOffset, u, 0.06);
          parts.push(
            part(
              `pilaster-${side.key}-${level}-${bay}`,
              "pilaster",
              "trim",
              [px, py, zc],
              isX ? [0.34, 0.14, height] : [0.14, 0.34, height],
            ),
          );
        }
      } else if (detail === "refend") {
        const [bx, by] = sidePoint(side, faceOffset, 0, 0.04);
        parts.push(
          part(
            `refend-${side.key}-${level}`,
            "refend",
            "trim",
            [bx, by, base + 0.12],
            isX
              ? [halfSpan * 2 + 0.16, 0.14, 0.24]
              : [0.14, halfSpan * 2 + 0.16, 0.24],
          ),
        );
      }
    }
  }

  // 檐口 + 上沿
  parts.push(
    part(
      "cornice",
      "cornice",
      "trim",
      [0, 0, top + 0.25],
      [W + 0.7, D + 0.7, 0.5],
    ),
  );
  parts.push(
    part(
      "cornice-lip",
      "cornice",
      "trim",
      [0, 0, top + 0.62],
      [W + 1.0, D + 1.0, 0.18],
    ),
  );

  if (parameters.roofStyle === "mansard") {
    parts.push(
      part(
        "mansard-1",
        "mansardStep",
        "roof",
        [0, 0, top + 1.2],
        [W + 0.15, D + 0.15, 1.3],
      ),
    );
    parts.push(
      part(
        "mansard-2",
        "mansardStep",
        "roof",
        [0, 0, top + 2.5],
        [Math.max(2, W - 2.2), Math.max(2, D - 2.2), 1.25],
      ),
    );
    parts.push(
      part(
        "roof-cap",
        "roofSlab",
        "roof",
        [0, 0, top + 3.25],
        [Math.max(1, W - 3.6), Math.max(1, D - 3.6), 0.25],
      ),
    );
    // 老虎窗：正面每开间一扇（出挑体量；窗面由贴图表达）
    const dormerBays = Math.min(parameters.baysX, 6);
    for (let bay = 0; bay < dormerBays; bay += 1) {
      const x = (bay - (dormerBays - 1) / 2) * Math.min(BAY, W / dormerBays);
      parts.push(
        part(
          `dormer-${bay + 1}`,
          "dormer",
          "roof",
          [x, hd + 0.2, top + 1.6],
          [0.95, 0.7, 1.15],
        ),
      );
    }
  } else {
    parts.push(
      part(
        "roof-slab",
        "roofSlab",
        "roof",
        [0, 0, top + 0.85],
        [W - 0.3, D - 0.3, 0.35],
      ),
    );
    // 屋顶设备（seed 决定有无与位置）
    if (rng() < 0.75) {
      const bx = (rng() * 2 - 1) * Math.max(0.5, hw - 2.2);
      const by = (rng() * 2 - 1) * Math.max(0.5, hd - 1.8);
      parts.push(
        part(
          "bulkhead",
          "bulkhead",
          "wall",
          [bx, by, top + 1.2 + 1.15],
          [Math.min(3, W * 0.4), Math.min(2.2, D * 0.4), 2.3],
        ),
      );
    }
    if (rng() < 0.6) {
      const ax = (rng() * 2 - 1) * Math.max(0.5, hw - 1.5);
      const ay = (rng() * 2 - 1) * Math.max(0.5, hd - 1.5);
      parts.push(
        part(
          "roof-ac",
          "bulkhead",
          "trim",
          [ax, ay, top + 1.2],
          [1.4, 1.0, 0.7],
        ),
      );
    }
  }

  return parts;
}
