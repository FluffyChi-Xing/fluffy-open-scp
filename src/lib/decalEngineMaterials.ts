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
import sdfFrag from "@/assets/shaders/decal/sdf.frag.glsl?raw";
/** 与 refinedRender.SunEnvRefs 的结构子集（避免 lib→pages 反向依赖）。 */
export interface EngineEnvRefs {
  sunDir: { value: ThreeNamespace.Vector3 };
  sunColor: { value: ThreeNamespace.Color };
  dayLight: { value: number };
  /** 夜间压暗因子（1 = 白天全亮，~0.15 = 夜间环境光近似）：
   * 引擎 decal 走延迟光照，夜间只剩环境项——平涂 shader 无光照响应，
   * 恒 1 会让所有贴花夜间"自发光"（2026-10-04 问题3）。 */
  nightBoost: { value: number };
  /** 霓虹动画时钟（秒，引擎 gameInfo.time 的墙钟近似）：SDF 族
   * decalLightBackground 的 uTime。场景无动画 decal 时可缺省（恒 0）。 */
  time?: { value: number };
}

export type EngineFamily = "sign" | "clip" | "hole" | "holo" | "sdf";

const FRAG: Record<EngineFamily, string> = {
  sign: signFrag,
  clip: clipFrag,
  hole: holeFrag,
  holo: holoFrag,
  // SDF 霓虹管链（decalLightBackground → Disabled 调光 → Darken → LightSDF
  // → NeonTube）：动画由 uTime/uDecalMaterialInfo/uDecalNUS uniform 驱动。
  sdf: sdfFrag,
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
  /** 引擎 decalMaterialInfo.xyz = lot 侧 material_data 三元组（2026-10-05
   * 取证定谳，docs/re/decal-engine-alignment.md §十五）：
   * x → 灯强 materialLightScale = x×16+0.25；y = **animSpeed 跑马灯速度**；
   * z → 灯管亮度 materialTubeLightFactor = z×8+1。SDF 族必填。 */
  materialInfo?: [number, number, number];
  /** 供电状态 → decalMaterialInfo.w（断电 = 霓虹半亮 lerp 0.5）。 */
  powered?: boolean;
  /** SDF 族盒世界尺寸 (sizeX, sizeY, sphereHeight)——引擎 decalNUS
   * （In.texcoord<t0> 顶点流语义；sphereHeight 暂取 sizeY 近似，待对拍校准）。 */
  nus?: [number, number, number];
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
  const [miX, miY, miZ] = opts.materialInfo ?? [1, 0, 0];
  // SDF 族的 decalMaterialData 语义不同于其他族，且**矩阵按列重组**——
  // 引擎 VS decalMaterialData4（容器 line 4139）把四行字典 colors 转置：
  //   列 0~2 = 输出 R/G/B 对四个掩码通道的**权重列**（Darken 的
  //   dot(data[i], lightScales) = 输出通道 i 加权和，不是"灯管颜色"）；
  //   列 3（w 列）= 四路独立动画参数（符号选 UV 轴 / 整数 = 分块数 /
  //   小数 = 相位，供 decalLightBackground）。
  // 实证（条目 0x23D05B09）：行 w = ±60.0/.3/.6/.9 —— 四路 60 块相位
  // 错开的追逐灯；不转置时 ±60 的 w 直接进 dot → 输出被 ±60×lsA 撑爆，
  // 整牌饱和成纯色块（六轮对拍"闪烁色块"根因）。
  // DTO colors 统一做过线性×2（量化链口径），引擎 SDF 链消费字典原始值
  // ——动画参数被 ×2 会破坏 chunks/offsets 编码，故 /2 还原。
  const materialDataRows =
    family === "sdf" && opts.layerColors
      ? [0, 1, 2, 3].map(
          (i) =>
            new THREE.Vector4(
              (opts.layerColors?.[0]?.[i] ?? 0) / 2,
              (opts.layerColors?.[1]?.[i] ?? 0) / 2,
              (opts.layerColors?.[2]?.[i] ?? 0) / 2,
              (opts.layerColors?.[3]?.[i] ?? 0) / 2,
            ),
        )
      : [
          new THREE.Vector4(kSun, kLight, 0, 0),
          new THREE.Vector4(0, 0, 0, 0),
          new THREE.Vector4(0, 0, 0, 0),
          new THREE.Vector4(0, 0, 0, 0),
        ];
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
      uDecalMaterialData: { value: materialDataRows },
      uDecalMaterialInfo: {
        value: new THREE.Vector4(
          miX,
          miY,
          miZ,
          opts.powered === false ? 0 : 1,
        ),
      },
      // SDF 族盒世界尺寸（引擎 decalNUS 顶点流的 uniform 等价物）
      uDecalNUS: { value: new THREE.Vector3(...(opts.nus ?? [1, 1, 1])) },
      // 霓虹动画时钟：直接引用 env 共享对象（rAF 统一推进，免遍历材质）
      uTime: opts.env.time ?? { value: 0 },
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
