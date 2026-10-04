/**
 * 引擎 GLSL 资产的 decal 材质工厂（sc-shader 管线产物）。
 *
 * 管线：crates/sc-shader（容器片段 → 人工清理 → HLSL 子集→GLSL 转译 →
 * 家族组合）产出 src/assets/shaders/decal/。本工厂只做 uniform 接线：
 * - 引擎常量（decalMaterialData/texXform/朝向）来自 lot 数据与运行时捕获
 *   定谳的语义（docs/re/runtime-capture-2026-10-01.md）；
 * - 光照输入直接引用 env 的 uniform 对象（昼夜热切换联动，与建筑注入材质
 *   同机制）。
 */
import type * as ThreeNamespace from "three";
import signFrag from "@/assets/shaders/decal/sign.frag.glsl?raw";
import signVert from "@/assets/shaders/decal/sign.vert.glsl?raw";
import clipFrag from "@/assets/shaders/decal/clip.frag.glsl?raw";
import holeFrag from "@/assets/shaders/decal/hole.frag.glsl?raw";
import holeVert from "@/assets/shaders/decal/hole.vert.glsl?raw";
import holoFrag from "@/assets/shaders/decal/holo.frag.glsl?raw";
/** 与 refinedRender.SunEnvRefs 的结构子集（避免 lib→pages 反向依赖）。 */
export interface EngineEnvRefs {
  sunDir: { value: ThreeNamespace.Vector3 };
  sunColor: { value: ThreeNamespace.Color };
  dayLight: { value: number };
  /** 夜间压暗因子（1 = 白天全亮，~0.15 = 夜间环境光近似）：
   * 引擎 decal 走延迟光照，夜间只剩环境项——平涂 shader 无光照响应，
   * 恒 1 会让所有贴花夜间"自发光"（2026-10-04 问题3）。 */
  nightBoost: { value: number };
}

export type EngineFamily = "sign" | "clip" | "hole" | "holo" | "sdf";

const FRAG: Record<EngineFamily, string> = {
  sign: signFrag,
  clip: clipFrag,
  hole: holeFrag,
  holo: holoFrag,
  sdf: holoFrag, // TODO: sdf.frag 需动画 uniform 驱动，先复用 holo 兜底
};
const VERT: Record<EngineFamily, string> = {
  sign: signVert,
  clip: signVert,
  // 破洞 = 体积盒 VS（盒局部归一化 + 真实进深 z → 视差内景）；
  // 此前复用 quad VS（z 恒 0）导致视差收缩退化为常量 = "假内景未实装"。
  hole: holeVert,
  holo: signVert,
  sdf: signVert,
};

export interface EngineMaterialOptions {
  /** 贴花贴图（per-entry raster：sign = raw 四通道掩码，直传给片元合成）。 */
  map: ThreeNamespace.Texture;
  /** 层颜色 Color1-4（线性×2，[4][4]）——量化合成的 GLSL 上色输入。 */
  layerColors?: number[][];
  /** 引擎 decalMaterialData[0].xy = (kSunContribution, kLightAmount)。 */
  decalData: [number, number] | null;
  /** 贴花世界朝向（transform 基第三行）。 */
  worldDirection: ThreeNamespace.Vector3;
  /** 与建筑注入材质共享的 env uniform 对象（昼夜热切换联动）。 */
  env: EngineEnvRefs;
  /** 渲染面（体积盒用 BackSide，quad 用 DoubleSide）。 */
  side?: ThreeNamespace.Side;
  /** alpha 全零实心图 → 不透明渲染（海报式）；缺省 → 引擎混合态。 */
  alphaZero?: boolean;
  /** 破洞体积盒的 (半宽, 半高, 半深)——hole 体积 VS 的 uBoxHalf。 */
  boxHalf?: [number, number, number];
}

/**
 * 组装引擎家族材质。所有引擎常量 uniform 一次性赋值；光照/昼夜 uniforms
 * 直接引用 env 对象实现热切换。
 */
export function createEngineDecalMaterial(
  THREE: typeof ThreeNamespace,
  family: EngineFamily,
  opts: EngineMaterialOptions,
): ThreeNamespace.ShaderMaterial {
  const [kSun, kLight] = opts.decalData ?? [0, 0];
  return new THREE.ShaderMaterial({
    uniforms: {
      uSampler0: { value: opts.map },
      uLayerColors: {
        value: [0, 1, 2, 3].map(
          (k) =>
            new THREE.Vector4(
              opts.layerColors?.[k]?.[0] ?? 0,
              opts.layerColors?.[k]?.[1] ?? 0,
              opts.layerColors?.[k]?.[2] ?? 0,
              opts.layerColors?.[k]?.[3] ?? 0,
            ),
        ),
      },
      uNightBoost: opts.env.nightBoost,
      // 破洞体积盒归一化（hole 体积 VS 消费；其他家族忽略）
      uBoxHalf: {
        value: new THREE.Vector3(...(opts.boxHalf ?? [1, 1, 1])),
      },
      uDecalMaterialData: {
        value: [
          new THREE.Vector4(kSun, kLight, 0, 0),
          new THREE.Vector4(0, 0, 0, 0),
          new THREE.Vector4(0, 0, 0, 0),
          new THREE.Vector4(0, 0, 0, 0),
        ],
      },
      uDecalMaterialInfo: { value: new THREE.Vector4(1, 1, 0, 1) },
      // per-entry raster = 预裁剪 atlas cell → texXform 恒等
      uTexXform: { value: new THREE.Vector4(1, 1, 0, 0) },
      uDecalWorldDirection: { value: opts.worldDirection.clone().normalize() },
      uWorldNormal: { value: opts.worldDirection.clone().normalize() },
      uWorldCameraDirection: { value: new THREE.Vector3(0, 0, 1) },
      uShadow: { value: 1 },
      // decalClipBack 常量标定（容器原文：specStrength 0 / specE 16 / gloss 0.06）
      uGloss: { value: 0.06 },
      uReflectance: { value: 0 },
      uSpecE: { value: 16 },
      uSpecStrength: { value: 0 },
      uAmbientDiff: { value: new THREE.Vector3(0.1, 0.11, 0.15) },
      uDayLight: opts.env.dayLight,
      uSpecularScale: { value: 0.6 },
      uSunDir3: opts.env.sunDir,
      uSunColor3: opts.env.sunColor,
      uAnimRatio: { value: 0 },
      uAnimResults: { value: new THREE.Vector4(-1, -1, -1, -1) },
      uUseV: { value: new THREE.Vector4(0, 0, 0, 0) },
    },
    vertexShader: VERT[family],
    fragmentShader: FRAG[family],
    side: opts.side ?? THREE.DoubleSide,
    // 量化合成产物 = 硬 alpha 裁剪 → 标准 alpha 混合即正确
    blending: THREE.NormalBlending,
    transparent: true,
    depthWrite: false,
    polygonOffset: true,
    polygonOffsetFactor: -4,
    polygonOffsetUnits: -4,
  });
}
