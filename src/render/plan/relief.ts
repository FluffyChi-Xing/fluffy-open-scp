/**
 * 建筑浮雕（building4 relief）计划：把「是否启用 / 常量 / 高度可用性」
 * 收敛为纯函数，供 exec 着色器与材质层共用。
 *
 * 常量来自发行版 `building4ClipAndReliefMapPS` 完整片段
 * （tmp/building_shaders_full.txt:896-935）：
 *   kReliefDepth = 0.16、kMaxConeRatio = 0.5、kFlatLevelInt = 23、
 *   kConeSteps = 8、kBinarySteps = 2、kCullDistanceSq = 16000（≈126m）。
 * 注意：同文件里另有一个 authoring 变体（depth 0.1、cull 160000、
 * reliefMap 返回原 tc），不能与发行版常量混用。
 */

export const RELIEF_CONSTANTS = {
  /** 距离平方剔除：发行版 16000 → ≈126m。 */
  cullDistanceSq: 16000,
  /** 视差深度（归一化切线空间）。 */
  depth: 0.16,
  /** 高度 ≤ 23/255 视为平面（作为 tc 偏移，不是提前退出）。 */
  flatLevelInt: 23,
  maxConeRatio: 0.5,
  coneSteps: 8,
  binarySteps: 2,
} as const;

export interface ReliefPlanInput {
  /** 用户/属性面板的浮雕开关。 */
  readonly toggle: boolean;
  /** 该材质 shaderMap.r 是否真的存在高度变化（平面建筑无浮雕）。 */
  readonly hasHeight: boolean;
}

export interface ReliefPlan {
  readonly enabled: boolean;
  readonly cullDistanceSq: number;
  readonly depth: number;
  readonly flatLevel: number;
  readonly maxConeRatio: number;
  readonly coneSteps: number;
  readonly binarySteps: number;
}

export function planRelief(input: ReliefPlanInput): ReliefPlan {
  return {
    enabled: input.toggle && input.hasHeight,
    cullDistanceSq: RELIEF_CONSTANTS.cullDistanceSq,
    depth: RELIEF_CONSTANTS.depth,
    flatLevel: RELIEF_CONSTANTS.flatLevelInt / 255,
    maxConeRatio: RELIEF_CONSTANTS.maxConeRatio,
    coneSteps: RELIEF_CONSTANTS.coneSteps,
    binarySteps: RELIEF_CONSTANTS.binarySteps,
  };
}

/**
 * 从 shaderMap 的 RGBA 像素判断是否存在浮雕高度：R > 23/255 的像素
 * 占比 ≥ threshold。纯函数，便于用合成像素测试。
 */
export function reliefHasHeight(
  rgba: Uint8Array | Uint8ClampedArray,
  threshold = 0.01,
): boolean {
  if (rgba.length < 4) return false;
  let elevated = 0;
  let total = 0;
  for (let i = 0; i + 3 < rgba.length; i += 4) {
    total += 1;
    if (rgba[i] > RELIEF_CONSTANTS.flatLevelInt) elevated += 1;
  }
  return total > 0 && elevated / total >= threshold;
}
