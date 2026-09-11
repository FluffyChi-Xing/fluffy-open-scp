import { pngBlobUrl } from "@/lib/three-gltf";
import type * as ThreeNamespace from "three";
import type { LotModelPayload } from "@/api/tauri";

/**
 * SCP 精细渲染模块（scene contributor）：building4 tint 着色器注入、
 * 5d 日/夜环境 uniform、tint 贴图预载与材质装配、逐 mesh 材质贴图绑定。
 * 从 PropertyEditorViewport 原地抽出（PE-重构-1），逻辑未变。
 */

type LotMaterial = NonNullable<LotModelPayload["materials"]>[number];

/**
 * specularity 通道：经多 package 比对（用户结论 2026-09-10），强制 B 通道
 * （窗）综合效果最佳，精细渲染固定使用；通道实验 UI 已停用（见
 * PropertyEditor.vue 注释），specMode=2 保留为将来复验的常量。
 */
export function effectiveSpecMode(): number {
  return 2;
}

/**
 * 5d 日/夜环境 uniform 共享实例（一次 rebuild 一组，全部 tint 材质引用同一
 * 对象，时段/供电变化直接改写免重建）。着色器侧 uSunDir 为世界空间。
 */
export type SunEnvRefs = {
  sunDir: { value: ThreeNamespace.Vector3 };
  sunColor: { value: ThreeNamespace.Color };
  skyColor: { value: ThreeNamespace.Color };
  dayLight: { value: number };
  powered: { value: number };
  glow: { value: number };
};

/** 太阳地平线高度 −1..1（t=6/18 日出日落、12 正午、0/24 子夜）。 */
export const sunAltitude = (t: number) => Math.sin(((t - 6) / 12) * Math.PI);

/** 白昼因子 0..1（含晨昏过渡带）。 */
export const dayFactor = (timeOfDay: number) => {
  const alt = sunAltitude(timeOfDay);
  return Math.min(1, Math.max(0, (alt + 0.08) / 0.5));
};

export function createSunEnv(
  THREE: typeof ThreeNamespace,
): SunEnvRefs {
  return {
    sunDir: { value: new THREE.Vector3(0.35, 0.8, 0.45).normalize() },
    sunColor: { value: new THREE.Color(1.0, 0.95, 0.85) },
    skyColor: { value: new THREE.Color(0.3, 0.42, 0.55) },
    dayLight: { value: 1 },
    powered: { value: 1 },
    glow: { value: 6.0 },
  };
}

export function applySunEnv(
  env: SunEnvRefs,
  timeOfDay: number,
  powered: boolean | undefined,
) {
  const t = timeOfDay;
  const alt = sunAltitude(t);
  const day = dayFactor(t);
  const azRad = ((t / 24) * 360 + 180) * (Math.PI / 180);
  const el = Math.max(alt, -0.45);
  env.sunDir.value
    .set(Math.cos(el) * Math.sin(azRad), Math.sin(el), Math.cos(el) * Math.cos(azRad))
    .normalize();
  // 太阳色：地平线橙 → 正午白 / 夜间月光蓝；天空：day→dusk→night 三段
  const warm = Math.min(1, Math.max(0, alt / 0.32));
  if (alt >= 0) {
    env.sunColor.value.setRGB(
      1,
      0.55 + 0.42 * warm,
      0.28 + 0.62 * warm,
    );
  } else {
    env.sunColor.value.setRGB(0.14, 0.17, 0.26); // 月光
  }
  const skyDay = { r: 0.3, g: 0.42, b: 0.55 };
  const skyDusk = { r: 0.24, g: 0.18, b: 0.2 };
  const skyNight = { r: 0.016, g: 0.022, b: 0.05 };
  const r = skyNight.r + (skyDusk.r + (skyDay.r - skyDusk.r) * warm - skyNight.r) * day;
  const g = skyNight.g + (skyDusk.g + (skyDay.g - skyDusk.g) * warm - skyNight.g) * day;
  const b = skyNight.b + (skyDusk.b + (skyDay.b - skyDusk.b) * warm - skyNight.b) * day;
  env.skyColor.value.setRGB(r, g, b);
  env.dayLight.value = day;
  env.powered.value = powered === false ? 0 : 1;
  // 源码 interiorMap.a×16 为 HDR；观察器无 tonemap，白天压 2.5 / 夜间放开 16
  env.glow.value = 2.5 + (16 - 2.5) * (1 - day);
}

/** slot0 参数表 f32 → DataTexture（cols×4 RGBA Float，texelFetch 寻址）。 */
export function buildParamsTexture(
  THREE: typeof ThreeNamespace,
  material: LotMaterial,
): ThreeNamespace.DataTexture | null {
  if (!material.paramsF32 || material.paramCols === 0) return null;
  const texture = new THREE.DataTexture(
    material.paramsF32,
    material.paramCols,
    4,
    THREE.RGBAFormat,
    THREE.FloatType,
  );
  texture.minFilter = THREE.NearestFilter;
  texture.magFilter = THREE.NearestFilter;
  texture.generateMipmaps = false;
  texture.needsUpdate = true;
  return texture;
}

/** 一次 rebuild 的 tint 贴图解析结果（每材质一组）。 */
export interface TintTextureSet {
  tintTex: ThreeNamespace.Texture | null;
  paletteTex: ThreeNamespace.Texture | null;
  normalTex: ThreeNamespace.Texture | null;
  shaderTex: ThreeNamespace.Texture | null;
  interiorTex: ThreeNamespace.Texture | null;
  paramsTex: ThreeNamespace.DataTexture | null;
  paramCols: number;
}

/**
 * 预载全部材质的 tint/palette/normal/shader/interior/params 贴图。
 * uvKind=2（facade tint 着色器）：贴图**预加载完成**后才建材质——
 * 否则首次编译时 uniform 为 null，采样 alpha=0 → 全部 discard（模型隐形）。
 */
export async function loadTintTextures(
  THREE: typeof ThreeNamespace,
  materials: LotMaterial[],
  registerTextureUrl: (url: string) => void,
): Promise<TintTextureSet[]> {
  const loader = new THREE.TextureLoader();
  const loadTex = (bytes: Uint8Array<ArrayBuffer>) =>
    new Promise<ThreeNamespace.Texture>((resolve, reject) => {
      const url = pngBlobUrl(bytes);
      registerTextureUrl(url);
      loader.load(
        url,
        (texture) => {
          texture.wrapS = THREE.ClampToEdgeWrapping;
          texture.wrapT = THREE.ClampToEdgeWrapping;
          // tint/palette/normal 按后端 bake 同坐标系采样（原始 UV，行 0 =
          // PNG 首行），不做 three 默认的 flipY 翻转。
          texture.flipY = false;
          resolve(texture);
        },
        undefined,
        reject,
      );
    });
  return Promise.all(
    materials.map(async (material) => ({
      tintTex: material.tintPng ? await loadTex(material.tintPng) : null,
      paletteTex: material.palettePng ? await loadTex(material.palettePng) : null,
      normalTex: material.normalPng ? await loadTex(material.normalPng) : null,
      shaderTex: material.shaderPng ? await loadTex(material.shaderPng) : null,
      interiorTex: material.interiorPng ? await loadTex(material.interiorPng).then((t) => {
        // 游戏 interiorMapSampler 为 REPEAT 包装：房间选择偏移（可能为整数倍
        // scale）依赖回绕取样；ClampToEdge 会把越界采样钳成边缘纯色（绿/紫块）
        t.wrapS = THREE.RepeatWrapping;
        t.wrapT = THREE.RepeatWrapping;
        return t;
      }) : null,
      paramsTex: buildParamsTexture(THREE, material),
      paramCols: material.paramCols,
    })),
  );
}

/**
 * tint 着色器注入：逐像素复刻 building4 链（§27/§28 源码逐字）。
 * TEXCOORD_2 = facade 世界投影 UV（Float4.xy），TEXCOORD_1.xy = materialIndex
 * /255 + 内景随机种子。fragment：baseUv = frac(vTintUv)*regionXform.xy +
 * regionXform.zw → tint 查表 → palette 查色（色行+末行 surface 行）×(tint.b*2)，
 * A<0.5 镂空 discard；法线图同 UV 重采样。
 * 5a 材质质感：shaderMap 通道×2=specStrength（uSpecG 切 G 数据/B 源码）、
 * palette 色 a³×2048+1=specE、surface 行 a=reflectance、gloss、AO=normalMap.a；
 * SimCityLighting 太阳 Blinn-Phong-Schlick 高光 + EnvLighting 常数天空近似。
 * 5b 假内景（ClipAndReliefMapPS/InteriorMapPS）：shaderMap.a=窗洞混合因子，
 * uv*roomInvSize 栅格化逐窗格 FastNoise 选房（4 变体×象限），interiorMap()
 * 盒体投影进 slot5 房间图集（interiorScale/Offset=row0.zw，roomInvSize=
 * row3.zw），夜灯=interiorMap.a×uInteriorGlow。
 * 30.2 Top 层双采样（TEXCOORD_3 = Float4.zw = uv2）：topUv =
 * frac(vTopUv)*regionXform2(row2)+offset，facadeTint.a@topUv = 窗户 motif
 * 覆盖率，shaderMap/normalMap/palette(palU2 列)/surface/亮度全部按其 lerp
 * ——公寓楼窗标记只在 Top 域（facade_survey 普查 86%），Base-only 采样
 * 会导致窗户全墙化。
 * 偏离源码处（均文档化）：①下向面豁免镂空（原版瑕疵）；②specularity 取 G
 * 通道（资产实证）；③内景自发光 16→uInteriorGlow 可调（无 HDR tonemap）；
 * ④interiorThresholds 用常数四分位（引擎值未知）；⑤eyeDir 用对象空间近似
 * 切线空间。
 */
export function attachTintShader(
  material: ThreeNamespace.MeshStandardMaterial,
  uniforms: {
    tintMap: { value: ThreeNamespace.Texture };
    paletteMap: { value: ThreeNamespace.Texture };
    shaderMapMap: { value: ThreeNamespace.Texture | null };
    interiorMapMap: { value: ThreeNamespace.Texture | null };
    paramsMap: { value: ThreeNamespace.Texture | null };
    uParamCols: { value: number };
    uSunDir: { value: ThreeNamespace.Vector3 };
    uSunColor: { value: ThreeNamespace.Color };
    uSkyColor: { value: ThreeNamespace.Color };
    uSpecMode: { value: number };
    uInteriorGlow: { value: number };
    /** 5d 日/夜：白昼因子 0..1（夜间环境/漫反射压暗、内景环境光） */
    uDayLight: { value: number };
    /** 5d 供电：0 = 内景自发光全灭（源码 interiorThresholds.z） */
    uPowered: { value: number };
  },
  paramsReady: boolean,
  shaderMapReady: boolean,
  interiorReady: boolean,
) {
  // 注意：three 默认编译为 GLSL ES 1.00——texelFetch/ivec2 不可用，
  // 参数表用 texture2D + 预计算 V 寻址（Nearest 采样取整行）。
  material.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, uniforms);
    shader.vertexShader = shader.vertexShader
      .replace(
        "#include <common>",
        `#include <common>
attribute vec4 uv1;
attribute vec2 uv2;
attribute vec2 uv3;
uniform float uParamCols;
varying vec2 vTintUv;
varying vec2 vTopUv;
varying float vMatU;
varying float vObjUp;
varying float vSeed;
varying vec3 vObjEyeDir;
varying vec3 vModelPos;`,
      )
      .replace(
        "#include <uv_vertex>",
        `#include <uv_vertex>
vTintUv = uv2;
vTopUv = uv3;
vMatU = (uv1.x * 255.0 + 0.5) / uParamCols;`,
      )
      .replace(
        "#include <beginnormal_vertex>",
        `#include <beginnormal_vertex>
vObjUp = normalize(objectNormal).z;
vSeed = uv1.y;
vModelPos = (modelMatrix * vec4(0.0, 0.0, 0.0, 1.0)).xyz;
{
  vec4 scMv = modelViewMatrix * vec4(position, 1.0);
  mat3 scNm = normalMatrix;
  mat3 scNmT = mat3(scNm[0][0], scNm[1][0], scNm[2][0],
                    scNm[0][1], scNm[1][1], scNm[2][1],
                    scNm[0][2], scNm[1][2], scNm[2][2]);
  vObjEyeDir = normalize(scNmT * normalize(-scMv.xyz));
}`,
      );
    shader.fragmentShader = shader.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
varying vec2 vTintUv;
varying vec2 vTopUv;
varying float vMatU;
varying float vObjUp;
varying float vSeed;
varying vec3 vObjEyeDir;
varying vec3 vModelPos;
uniform sampler2D tintMap;
uniform sampler2D paletteMap;
uniform vec3 uSunDir;
uniform vec3 uSunColor;
uniform vec3 uSkyColor;
uniform float uSpecMode;
uniform float uInteriorGlow;
uniform float uDayLight;
uniform float uPowered;
#ifdef TINT_PARAMS
uniform sampler2D paramsMap;
#endif
#ifdef TINT_SHADERMAP
uniform sampler2D shaderMapMap;
#endif
#ifdef TINT_INTERIOR
uniform sampler2D interiorMapMap;
// ClipAndReliefMapPS：盒体裁剪 + 透视投影（kInvDepth=0.5/kBackSize=0.5/kDilation=0.9）
vec2 scInteriorMap(vec3 eye, vec2 tc, float invMapDepth, float backSize, float dilation) {
  vec3 eyeDir = eye;
  eyeDir.z *= invMapDepth;
  vec3 pos = vec3(tc, 0.0) * -2.0 + 1.0;
  pos.z -= 1.0;
  vec3 k = (sign(eyeDir) - pos) / eyeDir;
  float t = min(k.x, min(k.y, k.z));
  vec3 target = pos + t * eyeDir;
  target.xy *= mix(dilation, backSize, target.z);
  return target.xy * -0.5 + 0.5;
}
// 源码 FastNoise 逐字
float scFastNoise(vec3 seed) {
  seed *= vec3(78.233, 12.9898, 43758.5453);
  seed += vec3(0.819 * 78.233, 0.819 * 12.9898, 0.819 * 43758.5453);
  return fract(seed.z * fract(seed.x * fract(seed.y)));
}
#endif`,
      )
      .replace(
        "#include <map_fragment>",
        `#include <map_fragment>
        #ifdef TINT_PARAMS
        vec4 xform = texture2D(paramsMap, vec2(vMatU, 0.375));
        vec4 xform2 = texture2D(paramsMap, vec2(vMatU, 0.625)); // row2=regionXform2(Top 层)
        vec4 palOrigin = texture2D(paramsMap, vec2(vMatU, 0.125));
        vec4 scRoom = texture2D(paramsMap, vec2(vMatU, 0.875)); // row3=(tilePadding.xy, roomInvSize.zw)
        #else
        vec4 xform = vec4(1.0, 1.0, 0.0, 0.0);
        vec4 xform2 = vec4(0.0);
        vec4 palOrigin = vec4(0.0);
        vec4 scRoom = vec4(0.0);
        #endif
        vec2 tUv = fract(vTintUv) * xform.xy + xform.zw;
        vec4 tintValues = texture2D(tintMap, tUv);
        // 30.2 Top 层（relief_tc 域，uv2×regionXform2）：窗户 motif 所在。
        // 源码（cpp frac 变体定谳）：tilePadding=row3.xy，且
        // reliefSrc = frac(uv2)·(1+padding) − padding/2，越出 [0,1] →
        // outsideTile>0 → facadeTint.a 强制 0（退回 Base 层）。玻璃幕墙楼
        // padding 高达 ~8e4（数值即语义：整体禁用 Top），公寓楼 ~(0.125,0)。
        vec2 topUv = vec2(0.0);
        float scFacade = 0.0;
        vec4 facadeTintValues = vec4(0.0);
        if (xform2.x > 0.0 && xform2.y > 0.0) {
          vec2 scPad = scRoom.xy;
          vec2 reliefSrc = fract(vTopUv) * (1.0 + scPad) - scPad * 0.5;
          float outsideTile =
            max(-reliefSrc.x, 0.0) + max(-reliefSrc.y, 0.0) +
            max(reliefSrc.x - 1.0, 0.0) + max(reliefSrc.y - 1.0, 0.0);
          topUv = clamp(reliefSrc, 0.0, 1.0) * xform2.xy + xform2.zw;
          facadeTintValues = texture2D(tintMap, topUv);
          scFacade = (outsideTile > 0.0) ? 0.0 : facadeTintValues.a;
        }
        vec2 scSubTop = facadeTintValues.rg * vec2(1.0 / 512.0, 1.0 / 16.0) + vec2(1.0 / 1024.0, 1.0 / 32.0);
        vec2 scSub = tintValues.rg * vec2(1.0 / 512.0, 1.0 / 16.0) + vec2(1.0 / 1024.0, 1.0 / 32.0);
        vec4 scPalColor = vec4(1.0);
        float scTintMul = tintValues.b * 2.0;
        float scExempt = 0.0;
        vec4 scShaderMap = vec4(1.0);
        if (tintValues.a < 0.5) {
          if (vObjUp >= -0.3) discard;
          // 下向面豁免（观察器缓解）：游戏 building4Clip 的镂空模板被地板/
          // 底面继承（底面与立面共用 facade UV），从下仰视出现穿透洞——
          // 游戏相机不可达此视角故原版未处理。豁免片段跳过调色保持白模观感。
          scExempt = 1.0;
        } else {
          // 源码 lerp(tintBase@palU, tintTop@palU2, facadeTint.a)：Top 层查
          // 调色板第二列（row0.y = palU2），亮度/子采样坐标同样取 Top 值
          vec4 scPalBase = texture2D(paletteMap, vec2(palOrigin.x + scSub.x, scSub.y));
          vec4 scPalTop = texture2D(paletteMap, vec2(palOrigin.y + scSubTop.x, scSubTop.y));
          scPalColor = mix(scPalBase, scPalTop, scFacade);
          scTintMul = mix(tintValues.b, facadeTintValues.b, scFacade) * 2.0;
          diffuseColor.rgb *= scPalColor.rgb * scTintMul;
          #ifdef USE_NORMALMAP
          // artistAO = normalMapSampled.a（Base/Top 双采样 lerp）
          float scAo = texture2D(normalMap, tUv).a;
          if (scFacade > 0.001) scAo = mix(scAo, texture2D(normalMap, topUv).a, scFacade);
          diffuseColor.rgb *= scAo;
          #endif
        }
        #ifdef TINT_SHADERMAP
        if (scExempt < 0.5) {
          vec4 smBase = texture2D(shaderMapMap, tUv);
          scShaderMap = smBase;
          if (scFacade > 0.001) {
            scShaderMap = mix(smBase, texture2D(shaderMapMap, topUv), scFacade);
          }
        }
        #endif
        // 5a spec 四标量（building4DeferredPS；色/表面行均 ×tintMul）。
        // specStrength 通道（uSpecMode）：0=自动逐像素（资产实证——墙面
        // specularity 在 .g、窗玻璃在 .b，与窗洞掩码 a 互补，如金样本墙面
        // G=168/窗洞 B=101.5）；1=强制 G；2=强制 B（源码字面）。
        float scSpecRaw = scShaderMap.b;
        if (uSpecMode < 0.5) {
          scSpecRaw = mix(scShaderMap.b, scShaderMap.g, step(0.5, scShaderMap.a));
        } else if (uSpecMode < 1.5) {
          scSpecRaw = scShaderMap.g;
        }
        float scSpecStrength = scSpecRaw * 2.0;
        float scSpecA = scPalColor.a * scTintMul;
        float scSpecE = scSpecA * scSpecA * scSpecA * 2048.0 + 1.0;
        float scGloss = clamp(scSpecA * scSpecStrength, 0.0, 1.0);
        vec4 scSurfaceBase = texture2D(paletteMap, vec2(palOrigin.x + scSub.x, 0.875 + scSub.y)); // kSurfacePalV
        vec4 scSurfaceTop = texture2D(paletteMap, vec2(palOrigin.y + scSubTop.x, 0.875 + scSubTop.y));
        vec4 scSurface = mix(scSurfaceBase, scSurfaceTop, scFacade);
        float scReflectance = scSurface.a * scTintMul;
        // 5b 假内景（ClipAndReliefMapPS + InteriorMapPS）：shaderMap.a = 窗洞
        // 混合因子（1=外观，0=透内景）。逐窗格栅格化 + FastNoise 选房
        // （4 变体 × 象限单元格），盒体投影进 slot5 房间图集。
        vec3 scInterior = vec3(0.0);
        float scOpacity = 1.0;
        #ifdef TINT_INTERIOR
        scOpacity = scShaderMap.a;
        if (scOpacity < 0.999) {
          float scSeedEff = vSeed;
          if (vSeed < 0.5) {
            // 源码：种子 <0.5 时按模型位置 munge
            vec2 scQ = floor(vModelPos.xy * 2.0) + floor(vModelPos.zz * 2.0);
            scSeedEff = scFastNoise(vec3(scQ, vSeed));
          }
          // 源码逐字：interiorUv = uv * regionXform.xy * interiorRoomInvSize。
          // roomInvSize = row3.zw、tilePadding = row3.xy（cpp frac 变体定谳；
          // 玻璃楼 padding~8e4 禁 Top / 公寓楼 (0.125,0) 两样本互证）。
          vec2 scInteriorUv = vTintUv * xform.xy * scRoom.zw;
          vec2 scInteriorElem = floor(scInteriorUv);
          vec2 scInteriorSrcUv = fract(scInteriorUv);
          // eyeDir 切线空间化（源码 eyeDir = -mul(tangentSpace, viewPos)，t1 插值）：
          // 用 vTintUv 的屏幕导数重建与立面 UV 轴对齐的切线架，把视线向量
          // （fragment→相机，view space）变换进 (u, v, normal) 基。盒体裁剪+
          // 透视投影（前 0.9/后 0.5）只有在该空间才成立。
          vec3 scPosV = -vViewPosition;
          vec3 scDpx = dFdx(scPosV);
          vec3 scDpy = dFdy(scPosV);
          vec2 scDuvx = dFdx(vTintUv);
          vec2 scDuvy = dFdy(vTintUv);
          float scDet = scDuvx.x * scDuvy.y - scDuvy.x * scDuvx.y;
          vec3 scTanU = vec3(1.0, 0.0, 0.0);
          vec3 scTanV = vec3(0.0, 1.0, 0.0);
          if (abs(scDet) > 1e-10) {
            scTanU = normalize((scDpx * scDuvy.y - scDpy * scDuvx.y) / scDet);
            scTanV = normalize((scDpy * scDuvx.x - scDpx * scDuvy.x) / scDet);
          }
          vec3 scEyeVecV = normalize(vViewPosition);
          vec3 scGeomN = normalize(vNormal);
          vec3 scEyeTan = vec3(
            dot(scEyeVecV, scTanU),
            dot(scEyeVecV, scTanV),
            dot(scEyeVecV, scGeomN)
          );
          vec2 scResultTc = scInteriorMap(scEyeTan * scRoom.zwz, scInteriorSrcUv, 0.5, 0.5, 0.9);
          vec2 scInteriorTc = scResultTc * vec2(palOrigin.z) + vec2(0.0, palOrigin.w);
          float scRoomId = scFastNoise(vec3(scInteriorElem, scSeedEff));
          float scRoomVariation = floor(scRoomId * 4.0);
          // interiorThresholds 引擎值未知，v1 用四分位（0.25/0.5/0.75）
          vec4 scEdge = vec4(step(vec3(0.25, 0.5, 0.75), vec3(scRoomId)), scRoomVariation * 4.0);
          scInteriorTc.x += dot(scEdge, vec4(1.0)) * palOrigin.z;
          vec4 scRoomTex = texture2D(interiorMapMap, scInteriorTc);
          // 内景照明：房间环境光随昼夜（夜间仅微光）+ 灯亮 a×glow（HDR×16
          // 的 tonemap 近似，白天压 2.5）×供电（断电全灭，源码 .z hack）
          float scSelfLight = scRoomTex.a * uInteriorGlow * uPowered;
          scInterior = scRoomTex.rgb * (mix(0.12, 1.0, uDayLight) + scSelfLight);
        }
        #endif
        diffuseColor.rgb = mix(scInterior, diffuseColor.rgb, scOpacity);`,
      )
      .replace(
        "#include <normal_fragment_maps>",
        `#include <normal_fragment_maps>
        #ifdef USE_NORMALMAP
        {
          vec2 nUv = fract(vTintUv) * xform.xy + xform.zw;
          mat3 tbn = getTangentFrame( - vViewPosition, nonPerturbedNormal, nUv );
          vec3 mapN = texture2D( normalMap, nUv ).xyz * 2.0 - 1.0;
          // Top 层法线（窗框/线脚凹凸）按 facadeTint.a lerp
          if (scFacade > 0.001) {
            vec3 nTop = texture2D( normalMap, topUv ).xyz * 2.0 - 1.0;
            mapN = mix(mapN, nTop, scFacade);
          }
          mapN.xy *= normalScale;
          normal = normalize( tbn * mapN );
        }
        #endif`,
      )
      .replace(
        "#include <lights_fragment_end>",
        `#include <lights_fragment_end>
        // 游戏 SimCityLighting（5a）：太阳 Blinn-Phong-Schlick 高光 + EnvLighting
        // 常数天空近似。源码半向量 = normalize(lightDir - viewDir)、能量归一
        // (specE+2)/8、Schlick exp2(-8.656170·cosLH)、specHighlight 额外叠加不经 tint。
        {
          vec3 scSunV = normalize((viewMatrix * vec4(uSunDir, 0.0)).xyz);
          vec3 scHalf = normalize(scSunV - normalize(vViewPosition));
          float scNDotH = clamp(dot(normal, scHalf), 0.0, 1.0);
          float scSpec = pow(scNDotH, max(scSpecE, 0.001)) * ((scSpecE + 2.0) / 8.0);
          float scSchlick = scReflectance + (1.0 - scReflectance) * exp2(-8.656170 * clamp(dot(scSunV, scHalf), 0.0, 1.0));
          float scSunMod = clamp(dot(scSunV, normal), 0.0, 1.0);
          reflectedLight.directSpecular += scSpec * scSchlick * scSpecStrength * scSunMod * uSunColor;
          reflectedLight.indirectSpecular += uSkyColor * (scGloss * 0.75) * diffuseColor.rgb * (1.0 - scExempt);
          // 5d 夜间：three 侧灯光的漫反射分量随白昼因子压暗（太阳高光/
          // 天空镜面已由 uSunColor/uSkyColor 变暗）
          float scNightDim = mix(0.22, 1.0, uDayLight);
          reflectedLight.directDiffuse *= scNightDim;
          reflectedLight.indirectDiffuse *= scNightDim;
        }`,
      );
  };
  material.customProgramCacheKey = () =>
    `building4-tint${shaderMapReady ? "+sm" : ""}${paramsReady ? "+pm" : ""}${interiorReady ? "+im" : ""}`;
}

/**
 * 为 uvKind=2 mesh 建 tint 材质（含着色器注入与共享环境 uniform）。
 * 返回 [材质, specMode uniform 引用]（引用供热切换收集）。
 */
export function makeTintMaterial(
  THREE: typeof ThreeNamespace,
  tint: TintTextureSet,
  env: SunEnvRefs,
): [ThreeNamespace.MeshStandardMaterial, { value: number }] {
  const tinted = new THREE.MeshStandardMaterial({
    roughness: 0.9,
    metalness: 0,
    side: THREE.DoubleSide,
    normalMap: tint.normalTex,
  });
  tinted.defines = { USE_UV: "" };
  if (tint.paramsTex) tinted.defines.TINT_PARAMS = "";
  if (tint.shaderTex) tinted.defines.TINT_SHADERMAP = "";
  const interiorReady = Boolean(tint.paramsTex && tint.shaderTex && tint.interiorTex);
  if (interiorReady) tinted.defines.TINT_INTERIOR = "";
  const uSpecGUniform = { value: effectiveSpecMode() };
  attachTintShader(
    tinted,
    {
      tintMap: { value: tint.tintTex! },
      paletteMap: { value: tint.paletteTex! },
      shaderMapMap: { value: tint.shaderTex },
      interiorMapMap: { value: tint.interiorTex },
      paramsMap: { value: tint.paramsTex },
      uParamCols: { value: tint.paramCols },
      // 5a/5d：太阳/天空/昼夜/供电为共享 uniform 实例（applySunEnv 热切换）
      uSunDir: env.sunDir,
      uSunColor: env.sunColor,
      uSkyColor: env.skyColor,
      uSpecMode: uSpecGUniform,
      uInteriorGlow: env.glow,
      uDayLight: env.dayLight,
      uPowered: env.powered,
    },
    Boolean(tint.paramsTex),
    Boolean(tint.shaderTex),
    interiorReady,
  );
  return [tinted, uSpecGUniform];
}

/**
 * 精细贴图：按 0x2001A 绑定的**每 mesh 材质**应用（遮罩红通道 baseColor +
 * 解 Swizzle 法线；可贴图判定服务端逐 mesh 给出）。异步加载，完成后按
 * isStale 守卫丢弃过期代。
 */
export function applyDeferredMaterialMaps(
  THREE: typeof ThreeNamespace,
  payload: LotModelPayload,
  materialGroups: ThreeNamespace.MeshStandardMaterial[][],
  registerTextureUrl: (url: string) => void,
  isStale: () => boolean,
) {
  const loader = new THREE.TextureLoader();
  const loadTexture = (
    bytes: Uint8Array<ArrayBuffer>,
    setup: (texture: ThreeNamespace.Texture) => void,
  ) => {
    const url = pngBlobUrl(bytes);
    registerTextureUrl(url);
    loader.load(
      url,
      (texture) => {
        if (isStale()) {
          texture.dispose();
          return;
        }
        setup(texture);
      },
      undefined,
      () => {},
    );
  };
  payload.materials?.forEach((material, materialIndex) => {
    const group = materialGroups[materialIndex] ?? [];
    if (!group.length) return;
    if (material.baseColorPng) {
      loadTexture(material.baseColorPng, (texture) => {
        texture.colorSpace = THREE.SRGBColorSpace;
        for (const refined of group) {
          refined.map = texture;
          refined.needsUpdate = true;
        }
      });
    }
    if (material.normalPng) {
      loadTexture(material.normalPng, (texture) => {
        for (const refined of group) {
          refined.normalMap = texture;
          refined.needsUpdate = true;
        }
      });
    }
    // shader map B 反转 = 粗糙度；normal alpha = AO（three 的 aoMap 读 R 通道）
    if (material.roughnessPng) {
      loadTexture(material.roughnessPng, (texture) => {
        for (const refined of group) {
          refined.roughnessMap = texture;
          refined.roughness = 1;
          refined.needsUpdate = true;
        }
      });
    }
    if (material.aoPng) {
      loadTexture(material.aoPng, (texture) => {
        for (const refined of group) {
          refined.aoMap = texture;
          refined.needsUpdate = true;
        }
      });
    }
  });
}
