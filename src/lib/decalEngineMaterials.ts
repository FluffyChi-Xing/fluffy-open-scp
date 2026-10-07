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
import neonLightFrag from "@/assets/shaders/decal/neon-light.frag.glsl?raw";
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
  /** 动态招牌开关（0 = 静态恒亮 / 1 = 扫掠动画）：SDF 族 uAnimEnabled。
   * 缺省 = 静态（精细渲染工具条"动态招牌"默认关）。 */
  animEnabled?: { value: number };
  /** 内景自发光峰值（破洞 kInteriorMapSelfLightMax）：白天 2.5（2026-09-30
   * 恒 16 白天过曝教训），夜间 ×nightBoost 与墙同步（2026-10-05 用户目视：
   * 游戏内破洞夜间不自发光）。不要绑建筑链的 glow——它 2026-10-05 起
   * 恒 16（HDR 由反照率/emissive 拆分回收），贴花平涂没有该链。 */
  decalGlow?: { value: number };
  /** @deprecated 旧字段名；建洞材质请用 decalGlow（见上）。 */
  glow?: { value: number };
}

export type EngineFamily = "sign" | "clip" | "hole" | "holo" | "sdf" | "neon-light";

const FRAG: Record<EngineFamily, string> = {
  sign: signFrag,
  clip: clipFrag,
  hole: holeFrag,
  holo: holoFrag,
  // SDF 霓虹管链（decalLightBackground → Disabled 调光 → Darken → LightSDF
  // → NeonTube）：动画由 uTime/uDecalMaterialInfo/uDecalNUS uniform 驱动。
  sdf: sdfFrag,
  "neon-light": neonLightFrag,
};
const VERT: Record<EngineFamily, string> = {
  sign: signVert,
  clip: signVert,
  // 破洞 = 投影 VS（与其他族同走 DecalGeometry 贴面路径，vTexcoord0 携带
  // 盒归一 xy、z 恒 +1 = 引擎背面光栅头对视角的盒底位置，天然贴合曲面）
  hole: holeVert,
  holo: signVert,
  sdf: signVert,
  "neon-light": holeVert,
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
  /** 深度测试/写入策略。悬浮广告牌需要写入深度以遮挡后方广告牌；
   * 投影 decal 保持只测试深度，避免破坏共面贴花排序。 */
  depthTest?: boolean;
  depthWrite?: boolean;
  /** 悬浮广告牌的透明像素不应占用深度；SDF 掩码使用极小阈值裁掉空白。 */
  alphaTest?: number;
  /** alpha 全零实心图 → 不透明渲染（海报式）；缺省 → 引擎混合态。 */
  alphaZero?: boolean;
  /** 涂鸦分流（uGraffiti）：1 = 喷漆连续厚度 alpha + ×uNightBoost（无灯箱
   * 增益、无夜间豁免——涂鸦 rt0 编译状态 = 标准 alpha 混合 + alphaTest
   * 0.02，无自发光项）；0/缺省 = 招牌灯箱。仅 sign 族消费。 */
  graffiti?: boolean;
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
  /** 破洞族盒参数（lot 局部坐标）：origin = 盒前缘（变换原点）、axisZ =
   * 盒进深轴、invDepth = 1/盒深——VS 计算 tfp.z 真实进深（引擎延迟路径
   * 深度重建的 CPU 同构：立面 −1 全尺寸内景、深部 +1 中心收缩）。
   * halfXY/depthM/invRot 供 PS 视线视差（holeParallaxUv：射线-盒底平面
   * 求交，空壳建筑的纵深来源）；camLot 每帧由 onBeforeRender 更新。 */
  holeBox?: {
    origin: ThreeNamespace.Vector3;
    axisZ: ThreeNamespace.Vector3;
    invDepth: number;
    halfXY?: [number, number];
    depthM?: number;
    invRot?: ThreeNamespace.Matrix3;
  };
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
  // sign 族缺省 x=0：自发光增益 clamp(x×16+0.25, 1, 4) 的缺省即 1（不
  // 发光、保持平涂）——缺数据时不误亮；sdf 族保持 x=1（霓虹灯强默认）。
  const [miX, miY, miZ] =
    opts.materialInfo ?? (family === "sign" ? [0, 0, 0] : [1, 0, 0]);
  // SDF 族的 decalMaterialData 语义不同于其他族，且**矩阵按列重组**——
  // 引擎 VS decalMaterialData4（容器 line 4139）把四行字典 colors 转置：
  //   列 0~2 = 输出 R/G/B 对四个掩码通道的**权重列**（Darken 的
  //   dot(data[i], lightScales) = 输出通道 i 加权和，不是"灯管颜色"）；
  //   列 3（w 列）= 四路独立动画参数（符号选 UV 轴 / 整数 = 分块数 /
  //   小数 = 相位，供 decalLightBackground）。
  // 实证（条目 0x23D05B09）：行 w = ±60.0/.3/.6/.9 —— 四路 60 块相位
  // 错开的追逐灯；不转置时 ±60 的 w 直接进 dot → 输出被 ±60×lsA 撑爆，
  // 整牌饱和成纯色块（六轮对拍"闪烁色块"根因）。
  // /2 口径（十一轮修正）：DTO colors 统一做过线性×2——**只有 w 列动画
  // 参数**需要 /2 还原（±120 → ±60，chunks/offsets 编码不被 ×2 破坏）；
  // 颜色权重列保持 DTO 满亮度（与静态分支 uLayerColors 同口径——此前四
  // 行一起 /2 = LED 比静态暗 2 倍的叠加因子，"淡到看不见"对拍）。
  const materialDataRows =
    (family === "sdf" || family === "neon-light") && opts.layerColors
      ? [0, 1, 2, 3].map(
          (i) =>
            new THREE.Vector4(
              (opts.layerColors?.[0]?.[i] ?? 0) / (i === 3 ? 2 : 1),
              (opts.layerColors?.[1]?.[i] ?? 0) / (i === 3 ? 2 : 1),
              (opts.layerColors?.[2]?.[i] ?? 0) / (i === 3 ? 2 : 1),
              (opts.layerColors?.[3]?.[i] ?? 0) / (i === 3 ? 2 : 1),
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
      // 涂鸦/招牌分流（仅 sign 族 PS 消费）
      uGraffiti: { value: opts.graffiti ? 1 : 0 },
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
      // SDF 族 sharp-bilinear 的纹素坐标输入（贴图像素尺寸；解码纹理在
      // 建材质时已就绪，缺省 64×64 不影响定性观感）
      uSdfTexSize: {
        value: new THREE.Vector2(
          (opts.map.image as { width?: number } | null)?.width ?? 64,
          (opts.map.image as { height?: number } | null)?.height ?? 64,
        ),
      },
      // 霓虹动画时钟：直接引用 env 共享对象（rAF 统一推进，免遍历材质）
      uTime: opts.env.time ?? { value: 0 },
      // 动态招牌开关（0 = 静态恒亮）：直引 env 共享对象，工具条切换即生效
      uAnimEnabled: opts.env.animEnabled ?? { value: 0 },
      // 破洞内景自发光峰值（kInteriorMapSelfLightMax）：白天 2.5、夜间
      // ×nightBoost 与墙同步（decalGlow 曲线在 applySunEnv）——不绑建筑
      // 链 glow（恒 16 白天爆平白、夜间自发光，双目视回归定谳）。
      uInteriorGlow: opts.env.decalGlow ?? opts.env.glow ?? { value: 16 },
      // 破洞族盒参数（VS 的 tfp.z 真实进深；缺省恒前缘 = 全尺寸内景）
      uHoleOrigin: { value: opts.holeBox?.origin.clone() ?? new THREE.Vector3() },
      uHoleAxisZ: {
        value: opts.holeBox?.axisZ.clone().normalize() ?? new THREE.Vector3(0, 0, 1),
      },
      uHoleInvDepth: { value: opts.holeBox?.invDepth ?? 0 },
      // 破洞视线视差（holeParallaxUv）：盒半宽/半高、盒深（米）、lot→贴花
      // 系旋转（基矩阵转置）；uHoleCamLot 每帧 onBeforeRender 写入（初值 =
      // 盒前缘正前方 1000m，未更新时不产生错误视差）
      uHoleHalfXY: {
        value: new THREE.Vector2(...(opts.holeBox?.halfXY ?? [1, 1])),
      },
      uHoleDepthM: { value: opts.holeBox?.depthM ?? 1 },
      uHoleInvRot: { value: opts.holeBox?.invRot ?? new THREE.Matrix3() },
      uHoleCamLot: {
        value: opts.holeBox
          ? opts.holeBox.origin
              .clone()
              .addScaledVector(opts.holeBox.axisZ, -1000)
          : new THREE.Vector3(0, 0, -1000),
      },
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
    blending: family === "neon-light" ? THREE.CustomBlending : THREE.NormalBlending,
    blendSrc: THREE.OneFactor,
    blendDst: THREE.OneFactor,
    transparent: true,
    depthTest: opts.depthTest ?? true,
    depthWrite: opts.depthWrite ?? false,
    alphaTest: opts.alphaTest ?? 0,
    polygonOffset: true,
    polygonOffsetFactor: -4,
    polygonOffsetUnits: -4,
  });
}
