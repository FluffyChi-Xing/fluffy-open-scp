<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { ThreeViewer, disposeObject } from "@/lib/three-viewer";
import { parseLotModelObjects, pngBlobUrl } from "@/lib/three-gltf";
import { composeRefinedGround } from "./refinedGround";
import type * as ThreeNamespace from "three";
import type { LotModelLodRef, LotModelPayload, LotUnitDto } from "@/api/tauri";
import type { ModelState, UnitGrouping } from "./usePropertyEditorSession";
import {
  buildPathLine,
  buildRealLightUnit,
  buildUnitObject,
  unitId,
} from "./unitGizmos";

const props = defineProps<{
  modelPayload: LotModelPayload | null;
  /** LOD1~LOD4 资源位置（index 0 = LOD1）；缺失级为 null。 */
  modelLods: (LotModelLodRef | null)[];
  activeLod: number;
  renderMode: "default" | "refined";
  grouping: UnitGrouping;
  lotSize: [number, number] | null;
  /** LotPlacementTransform 行主序 12 floats；地面矩形取其逆对齐建筑。 */
  lotPlacement: number[] | null;
  /** LotColor1-4 RGBA（A = 地面贴图索引 0-15）。 */
  lotColors: [number, number, number, number][];
  /** LotColor1-4 是否实际存在（false = 回退色，不参与着色）。 */
  lotColorsAuthored: boolean[];
  lotMaskPng: string | null;
  selectedId: string | null;
  hiddenUnits: Set<string>;
  groupVisibility: Record<string, boolean>;
  modelState: ModelState;
  /** 通道实验开关（仅精细模式显示）；关闭时恒用自动逐像素。 */
  specExperiment?: boolean;
  /** 0=自动逐像素（默认）/ 1=强制 G·墙 / 2=强制 B·窗（源码字面）。 */
  specMode?: number;
  /** 日/夜时段 0–24（默认 12 正午）；驱动太阳方向/颜色/天空/内景夜灯。 */
  timeOfDay?: number;
  /** 供电（默认 true）：断电 = 内景自发光全灭（源码 interiorThresholds.z hack）。 */
  powered?: boolean;
  /** Top 层浮雕：slot5 alpha 高度作 bumpMap（building4Clip reliefMap 近似）。 */
  reliefEnabled?: boolean;
}>();
const emit = defineEmits<{
  select: [id: string | null];
  "toggle-layer": [name: string];
  "switch-lod": [index: number];
}>();
useI18n();
const container = shallowRef<HTMLElement | null>(null);
const viewer = shallowRef<ThreeViewer | null>(null);
const sceneReady = ref(false);
const lightPanelOpen = ref(false);
const lodPanelOpen = ref(false);
const infoPanelOpen = ref(false);
const infoCopied = ref(false);
let infoCopiedTimer: ReturnType<typeof setTimeout> | undefined;
const lightAzimuth = ref(45);
const lightElevation = ref(55);
/** 日/夜模拟：全局环境亮度倍率（1 = 当前观感，0 ≈ 夜，2 ≈ 正午）。 */
const brightness = ref(1);

watch([lightAzimuth, lightElevation], () => {
  viewer.value?.setKeyLight(lightAzimuth.value, lightElevation.value);
});
watch(brightness, () => applyBrightness());

/** 存活 tint 材质的 uSpecMode uniform 引用（通道实验热切换，免重建）。 */
const specUniformRefs: { value: number }[] = [];
/** 精细材质引用（浮雕开关热切换 bumpScale，免重建）。 */
const refinedMaterialRefs: ThreeNamespace.MeshStandardMaterial[] = [];
/** 浮雕强度（约 kReliefDepth=0.1 的观感等效，经目视校准）。 */
const RELIEF_BUMP_SCALE = 0.35;
watch(
  () => props.reliefEnabled,
  (enabled) => {
    for (const material of refinedMaterialRefs) {
      material.bumpScale = enabled ? RELIEF_BUMP_SCALE : 0;
    }
  },
);

/**
 * specularity 通道：经多 package 比对（用户结论 2026-09-10），强制 B 通道
 * （窗）综合效果最佳，精细渲染固定使用；通道实验 UI 已停用（见
 * PropertyEditor.vue 注释），specMode=2 保留为将来复验的常量。
 */
const effectiveSpecMode = () => 2;
watch([() => props.specExperiment, () => props.specMode], () => {
  const value = effectiveSpecMode();
  for (const uniform of specUniformRefs) uniform.value = value;
});

/**
 * 5d 日/夜环境 uniform 共享实例（一次 rebuild 一组，全部 tint 材质引用同一
 * 对象，时段/供电变化直接改写免重建）。着色器侧 uSunDir 为世界空间。
 */
type SunEnvRefs = {
  sunDir: { value: ThreeNamespace.Vector3 };
  sunColor: { value: ThreeNamespace.Color };
  skyColor: { value: ThreeNamespace.Color };
  dayLight: { value: number };
  powered: { value: number };
  glow: { value: number };
};
let envRefs: SunEnvRefs | null = null;
/** 太阳地平线高度 −1..1（t=6/18 日出日落、12 正午、0/24 子夜）。 */
const sunAltitude = (t: number) => Math.sin(((t - 6) / 12) * Math.PI);
/** 白昼因子 0..1（含晨昏过渡带）。 */
const dayFactor = () => {
  const alt = sunAltitude(props.timeOfDay ?? 12);
  return Math.min(1, Math.max(0, (alt + 0.08) / 0.5));
};
function applySun() {
  if (!envRefs) return;
  const t = props.timeOfDay ?? 12;
  const alt = sunAltitude(t);
  const day = dayFactor();
  const azRad = ((t / 24) * 360 + 180) * (Math.PI / 180);
  const el = Math.max(alt, -0.45);
  envRefs.sunDir.value
    .set(Math.cos(el) * Math.sin(azRad), Math.sin(el), Math.cos(el) * Math.cos(azRad))
    .normalize();
  // 太阳色：地平线橙 → 正午白 / 夜间月光蓝；天空：day→dusk→night 三段
  const warm = Math.min(1, Math.max(0, alt / 0.32));
  if (alt >= 0) {
    envRefs.sunColor.value.setRGB(
      1,
      0.55 + 0.42 * warm,
      0.28 + 0.62 * warm,
    );
  } else {
    envRefs.sunColor.value.setRGB(0.14, 0.17, 0.26); // 月光
  }
  const skyDay = { r: 0.3, g: 0.42, b: 0.55 };
  const skyDusk = { r: 0.24, g: 0.18, b: 0.2 };
  const skyNight = { r: 0.016, g: 0.022, b: 0.05 };
  const r = skyNight.r + (skyDusk.r + (skyDay.r - skyDusk.r) * warm - skyNight.r) * day;
  const g = skyNight.g + (skyDusk.g + (skyDay.g - skyDusk.g) * warm - skyNight.g) * day;
  const b = skyNight.b + (skyDusk.b + (skyDay.b - skyDusk.b) * warm - skyNight.b) * day;
  envRefs.skyColor.value.setRGB(r, g, b);
  envRefs.dayLight.value = day;
  envRefs.powered.value = props.powered === false ? 0 : 1;
  // 源码 interiorMap.a×16 为 HDR；观察器无 tonemap，白天压 2.5 / 夜间放开 16
  envRefs.glow.value = 2.5 + (16 - 2.5) * (1 - day);
}
watch([() => props.timeOfDay, () => props.powered], () => {
  applySun();
  applyBrightness();
});

/** 复制模型槽位诊断（mesh/material/texture 及来源包关系），供复盘。 */
async function copyDiagnostics() {
  const text = props.modelPayload?.diagnostics ?? "";
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // 剪贴板 API 不可用（非安全上下文等）时的兜底
    const helper = document.createElement("textarea");
    helper.value = text;
    document.body.appendChild(helper);
    helper.select();
    document.execCommand("copy");
    helper.remove();
  }
  infoCopied.value = true;
  clearTimeout(infoCopiedTimer);
  infoCopiedTimer = setTimeout(() => {
    infoCopied.value = false;
  }, 1600);
}

const GROUP_KEYS = [
  "model",
  "lights",
  "props",
  "decals",
  "effects",
  "spawners",
  "paths",
] as const;

const unitObjects = new Map<string, ThreeNamespace.Object3D>();
/** 当前 payload 派生的贴图 blob URL；rebuild 时回收上一代。 */
const textureUrls: string[] = [];
/**
 * 场景真实光源总数上限：WebGL 前向渲染每个片元都评估全部光源，
 * 多灯地块（路灯密集的建筑群）推近镜头时片元数×光源数导致掉帧。
 * 超限时从整体强度最弱的单元开始摘除真实光源（保留透明拾取代理）。
 */
const MAX_REAL_LIGHTS = 24;
let rebuildToken = 0;

onMounted(async () => {
  const element = container.value;
  if (!element) return;
  viewer.value = await ThreeViewer.create(element, {
    onTap: (hit) => {
      let node: ThreeNamespace.Object3D | null = hit.object;
      while (node) {
        if (typeof node.userData?.unitId === "string") {
          emit("select", node.userData.unitId);
          return;
        }
        node = node.parent;
      }
      emit("select", null);
    },
  });
  await rebuild();
});
onBeforeUnmount(() => {
  rebuildToken += 1;
  viewer.value?.dispose();
  viewer.value = null;
  clearTimeout(infoCopiedTimer);
  unitObjects.clear();
  for (const url of textureUrls) URL.revokeObjectURL(url);
  textureUrls.length = 0;
});

function kindGroup(kind: LotUnitDto["kind"]) {
  return kind === "pathPoint" ? "paths" : `${kind}s`;
}

/** slot0 参数表 f32 → DataTexture（cols×4 RGBA Float，texelFetch 寻址）。 */
function buildParamsTexture(
  THREE: typeof ThreeNamespace,
  material: NonNullable<LotModelPayload["materials"]>[number],
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
function attachTintShader(
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

async function rebuild() {
  const instance = viewer.value;
  if (!instance) return;
  const token = ++rebuildToken;
  const THREE = instance.THREE;
  specUniformRefs.length = 0;
  for (const name of GROUP_KEYS) instance.clearGroup(name);
  unitObjects.clear();
  for (const url of textureUrls) URL.revokeObjectURL(url);
  textureUrls.length = 0;

  const payload = props.modelPayload;
  const modelObjects = payload ? await parseLotModelObjects(payload.glbs) : [];
  if (token !== rebuildToken) {
    for (const object of modelObjects) disposeObject(object);
    return;
  }
  const whiteMaterial = new THREE.MeshStandardMaterial({
    color: 0xb8c2cc,
    roughness: 0.55,
    metalness: 0.12,
    side: THREE.DoubleSide,
  });
  // 逐 mesh 材质下标（与 glbs 同序）；材质分组的下标 → 该组 mesh 的材质列表
  const materialGroups: ThreeNamespace.MeshStandardMaterial[][] = (
    payload?.materials ?? []
  ).map(() => []);
  // uvKind=2（facade tint 着色器）：贴图**预加载完成**后才建材质——
  // 否则首次编译时 uniform 为 null，采样 alpha=0 → 全部 discard（模型隐形）
  const loader = new THREE.TextureLoader();
  const loadTex = (bytes: Uint8Array<ArrayBuffer>) =>
    new Promise<ThreeNamespace.Texture>((resolve, reject) => {
      const url = pngBlobUrl(bytes);
      textureUrls.push(url);
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
  const tintResolved = await Promise.all(
    (payload?.materials ?? []).map(async (material) => ({
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
  if (token !== rebuildToken) return;
  // 5d 日/夜环境共享 uniform（全部 tint 材质引用同一组对象）
  const env: SunEnvRefs = {
    sunDir: { value: new THREE.Vector3(0.35, 0.8, 0.45).normalize() },
    sunColor: { value: new THREE.Color(1.0, 0.95, 0.85) },
    skyColor: { value: new THREE.Color(0.3, 0.42, 0.55) },
    dayLight: { value: 1 },
    powered: { value: 1 },
    glow: { value: 6.0 },
  };
  envRefs = env;
  for (const [index, object] of modelObjects.entries()) {
    const materialIndex = payload?.meshMaterialIndices[index] ?? 0;
    const uvKind = payload?.meshUvKinds[index] ?? 0;
    object.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (!mesh.isMesh) return;
      if (props.renderMode !== "refined") {
        mesh.material = whiteMaterial;
        return;
      }
      const tint = tintResolved[materialIndex];
      if (uvKind === 2 && tint?.tintTex && tint.paletteTex) {
        // facade tint 着色器：逐像素复刻 building4 链（tint 查表 → palette 查色）
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
        specUniformRefs.push(uSpecGUniform);
        attachTintShader(
          tinted,
          {
            tintMap: { value: tint.tintTex },
            paletteMap: { value: tint.paletteTex },
            shaderMapMap: { value: tint.shaderTex },
            interiorMapMap: { value: tint.interiorTex },
            paramsMap: { value: tint.paramsTex },
            uParamCols: { value: tint.paramCols },
            // 5a/5d：太阳/天空/昼夜/供电为共享 uniform 实例（applySun 热切换）
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
        mesh.material = tinted;
        return;
      }
      // GLB 的 COLOR_0（该 mesh 材质调色板的顶点色）→ GLTFLoader 的 color 属性
      const refined = new THREE.MeshStandardMaterial({
        vertexColors: Boolean(mesh.geometry.attributes.color),
        roughness: 0.82,
        metalness: 0,
        side: THREE.DoubleSide,
      });
      mesh.material = refined;
      if (uvKind === 1) materialGroups[materialIndex]?.push(refined);
    });
    instance.group("model").add(object);
  }
  // 精细贴图：按 0x2001A 绑定的**每 mesh 材质**应用（遮罩红通道 baseColor +
  // 解 Swizzle 法线；可贴图判定服务端逐 mesh 给出）
  if (props.renderMode === "refined" && payload) {
    const generation = token;
    const loader = new THREE.TextureLoader();
    const loadTexture = (
      bytes: Uint8Array<ArrayBuffer>,
      setup: (texture: ThreeNamespace.Texture) => void,
    ) => {
      const url = pngBlobUrl(bytes);
      textureUrls.push(url);
      loader.load(
        url,
        (texture) => {
          if (generation !== rebuildToken) {
            texture.dispose();
            return;
          }
          setup(texture);
        },
        undefined,
        () => {},
      );
    };
    payload.materials.forEach((material, materialIndex) => {
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
      // slot5 alpha 高度 → bumpMap 浮雕（开关只调 bumpScale，纹理常驻）
      if (material.reliefPng) {
        loadTexture(material.reliefPng, (texture) => {
          for (const refined of group) {
            refined.bumpMap = texture;
            refined.bumpScale = props.reliefEnabled ? RELIEF_BUMP_SCALE : 0;
            refined.needsUpdate = true;
            refinedMaterialRefs.push(refined);
          }
        });
      }
    });
  }

  // Lot 地面矩形（LotSize）；有 LotMask 时异步贴四色量化图。
  if (props.lotSize) {
    const ground = buildLotRect(THREE, props.lotSize);
    // C# CreateLotModel：地面按 LotPlacementTransform 的逆矩阵摆放——
    // 建筑在地块内不居中时，逆变换把遮罩图案对回建筑原点。
    // 地面 = LotPlacementTransform 的逆（mask 行列轴与模型 XY 轴直接对应）。
    // 注意：C# CreateLotModel 在此还叠加了 R(−90°Z)，但那是 WPF/Helix 视口
    // 约定的补偿——真实数据检验（lot_anchor_probe 轴长统计：无 placement
    // lot 741:48 支持 0°；0x4EF6F6CD 视觉实证）表明引擎约定不旋转，
    // 旋转会导致 mask 相对建筑转置（用户实测 2026-09-10）。
    if (props.lotPlacement) {
      const m = props.lotPlacement;
      const inverse = new THREE.Matrix4()
        .set(
          m[0], m[3], m[6], m[9],
          m[1], m[4], m[7], m[10],
          m[2], m[5], m[8], m[11],
          0, 0, 0, 1,
        )
        .invert();
      ground.matrix.copy(inverse);
    }
    // 关闭自动更新：矩阵完全由 placement 逆决定，防止渲染循环覆盖。
    ground.matrixAutoUpdate = false;
    instance.group("model").add(ground);
    if (props.lotMaskPng) {
      const generation = token;
      new THREE.TextureLoader().load(props.lotMaskPng, (texture) => {
        if (generation !== rebuildToken) {
          texture.dispose();
          return;
        }
        texture.colorSpace = THREE.SRGBColorSpace;
        // LotMask 为原始栅格行序（行 0 = 首行）：与模型贴图一致不翻 V，
        // 否则遮罩南北镜像（rendering.md §3.1 遗留项）
        texture.flipY = false;
        const fill = ground.children.find((child) => (child as ThreeNamespace.Mesh).isMesh) as
          | ThreeNamespace.Mesh
          | undefined;
        if (!fill) return;
        if (props.renderMode === "refined") {
          // 精细模式：引擎语义 = 每通道 LotColor.RGB 着色 × LotColor.A 索引的
          // 16 格地面贴图（shader baseTileUVMinMax 4×4 图集；C# lot editor 的
          // GroundTextures 下拉即此 Alpha）。此处按 8 tile/边近似平铺。
          composeRefinedGround(props.lotColors, props.lotColorsAuthored, texture.image, THREE)
            .then((map) => {
              if (generation !== rebuildToken || !map) {
                map?.dispose();
                return;
              }
              const material = fill.material as ThreeNamespace.MeshBasicMaterial;
              material.map = map;
              material.transparent = true;
              material.opacity = 1;
              material.color.set(0xffffff);
              material.needsUpdate = true;
            })
            .catch(() => {});
        } else {
          const material = fill.material as ThreeNamespace.MeshBasicMaterial;
          material.map = texture;
          material.transparent = false;
          material.opacity = 1;
          material.color.set(0xffffff);
          material.needsUpdate = true;
        }
      });
    }
  }

  const units: LotUnitDto[] = [
    ...props.grouping.lights,
    ...props.grouping.decals,
    ...props.grouping.props,
    ...props.grouping.effects,
    ...props.grouping.spawners,
    ...props.grouping.pathPoints,
  ];
  for (const unit of units) {
    // 精细模式：光源用真实 three.js 光源；其余组件保持标记锥
    const object =
      props.renderMode === "refined" && unit.kind === "light"
        ? buildRealLightUnit(THREE, unit)
        : buildUnitObject(THREE, unit);
    if (!object) continue;
    instance.group(kindGroup(unit.kind)).add(object);
    unitObjects.set(unitId(unit), object);
  }

  // 路径折线：按 point_index 排序连接（pathPairs 语义未定，先 best-effort）。
  const points = [...props.grouping.pathPoints]
    .filter((point) => point.point)
    .sort(
      (a, b) => (a.pointIndex ?? a.index) - (b.pointIndex ?? b.index),
    )
    .map((point) => new THREE.Vector3(...point.point!));
  const line = buildPathLine(THREE, points);
  if (line) instance.group("paths").add(line);

  pruneExcessLights(instance);

  instance.frameContent();
  instance.setKeyLight(45, 55);
  applySun();
  applyBrightness();
  applyGroupVisibility();
  applyUnitVisibility();
  applySelection();
  sceneReady.value = true;
}

/** 日/夜亮度：仅缩放环境四灯；地块真实光源保持常亮（夜间灯依然亮）。 */
function applyBrightness() {
  // 5d：夜间环境光随白昼因子压暗（亮度滑杆仍是用户侧总倍率）
  viewer.value?.setEnvironmentBrightness(brightness.value * (0.22 + 0.78 * dayFactor()));
}

/** 真实光源总数超限时，从强度最弱的单元开始摘除光源本体。 */
function pruneExcessLights(instance: ThreeViewer) {
  const perUnit: {
    lights: ThreeNamespace.Light[];
    intensity: number;
  }[] = [];
  let total = 0;
  for (const [id, object] of unitObjects) {
    if (!id.startsWith("light:")) continue;
    const lights: ThreeNamespace.Light[] = [];
    object.traverse((child) => {
      if ((child as ThreeNamespace.Light).isLight) {
        lights.push(child as ThreeNamespace.Light);
      }
    });
    if (!lights.length) continue;
    total += lights.length;
    perUnit.push({
      lights,
      intensity: lights.reduce((sum, light) => sum + light.intensity, 0),
    });
  }
  if (total <= MAX_REAL_LIGHTS) return;
  perUnit.sort((a, b) => a.intensity - b.intensity);
  for (const entry of perUnit) {
    for (const light of entry.lights) {
      if (total <= MAX_REAL_LIGHTS) return;
      light.parent?.remove(light);
      light.dispose();
      total -= 1;
    }
  }
}

/** Lot 地面矩形：XY 平面（Z-up 贴地面）细边框 + 半透明填充。 */
function buildLotRect(
  THREE: typeof ThreeNamespace,
  lotSize: [number, number],
): ThreeNamespace.Object3D {
  const [width, depth] = lotSize;
  const group = new THREE.Group();
  const half = [width / 2, depth / 2];
  const corners = [
    new THREE.Vector3(-half[0], -half[1], 0.05),
    new THREE.Vector3(half[0], -half[1], 0.05),
    new THREE.Vector3(half[0], half[1], 0.05),
    new THREE.Vector3(-half[0], half[1], 0.05),
  ];
  const border = new THREE.LineLoop(
    new THREE.BufferGeometry().setFromPoints(corners),
    new THREE.LineBasicMaterial({ color: 0x9aa4b8 }),
  );
  const fill = new THREE.Mesh(
    new THREE.PlaneGeometry(width, depth),
    new THREE.MeshBasicMaterial({
      color: 0x6b7689,
      transparent: true,
      opacity: 0.08,
      side: THREE.DoubleSide,
    }),
  );
  fill.position.z = 0.02;
  group.add(border, fill);
  return group;
}

function applyGroupVisibility() {
  const instance = viewer.value;
  if (!instance) return;
  for (const name of GROUP_KEYS) {
    instance.group(name).visible = props.groupVisibility[name] !== false;
  }
}
function applyUnitVisibility() {
  for (const [id, object] of unitObjects) {
    object.visible = !props.hiddenUnits.has(id);
  }
}
function applySelection() {
  const selected = props.selectedId
    ? unitObjects.get(props.selectedId) ?? null
    : null;
  viewer.value?.setSelected(selected);
}

watch(
  () => [props.modelPayload, props.renderMode, props.grouping],
  () => void rebuild(),
);
watch(() => props.groupVisibility, applyGroupVisibility, { deep: true });
watch(() => props.hiddenUnits, applyUnitVisibility);
watch(() => props.selectedId, applySelection);
</script>

<template>
  <div class="viewport-pane">
    <div ref="container" class="viewport-3d" />
    <div v-if="modelState === 'loading'" class="viewport-overlay viewport-status">
      <FSpinner size="sm" :label="$t('common.loading')" />
    </div>
    <p
      v-else-if="modelState === 'missing' || modelState === 'error'"
      class="viewport-overlay viewport-status"
      role="status"
    >
      {{
        modelState === "missing"
          ? $t("package.propertyEditorModelMissing")
          : $t("package.propertyEditorModelError")
      }}
    </p>
    <div class="viewport-overlay viewport-tools">
      <button
        type="button"
        :aria-label="$t('package.lodSwitch')"
        :title="$t('package.lodSwitch')"
        :aria-pressed="lodPanelOpen"
        :class="{ active: lodPanelOpen }"
        @click="lodPanelOpen = !lodPanelOpen"
      >
        <FIcon name="Layers" :size="13" aria-label="" />
      </button>
      <button
        type="button"
        :aria-label="$t('package.lightControls')"
        :title="$t('package.lightControls')"
        :aria-pressed="lightPanelOpen"
        :class="{ active: lightPanelOpen }"
        @click="lightPanelOpen = !lightPanelOpen"
      >
        <FIcon name="Lightbulb" :size="13" aria-label="" />
      </button>
      <button
        type="button"
        :aria-label="$t('package.modelInfo')"
        :title="$t('package.modelInfo')"
        :aria-pressed="infoPanelOpen"
        :class="{ active: infoPanelOpen }"
        @click="infoPanelOpen = !infoPanelOpen"
      >
        <FIcon name="Info" :size="13" aria-label="" />
      </button>
      <button
        type="button"
        :aria-label="$t('package.resetView')"
        :title="$t('package.resetView')"
        @click="viewer?.resetView()"
      >
        <FIcon name="RotateCcw" :size="13" aria-label="" />
      </button>
    </div>
    <div
      v-if="lodPanelOpen"
      class="viewport-overlay lod-mask"
      @click.self="lodPanelOpen = false"
    >
      <div class="lod-card" role="dialog" :aria-label="$t('package.lodSwitch')">
        <header class="lod-card-header">
          <span>{{ $t("package.lodSwitch") }}</span>
          <button
            type="button"
            class="lod-close"
            :aria-label="$t('common.close')"
            @click="lodPanelOpen = false"
          >
            <FIcon name="X" :size="13" aria-label="" />
          </button>
        </header>
        <div class="lod-tiles">
          <button
            v-for="(lod, index) in modelLods"
            :key="index"
            type="button"
            class="lod-tile"
            :class="{
              active: index === activeLod,
              missing: lod === null,
            }"
            :disabled="lod === null"
            @click="emit('switch-lod', index); lodPanelOpen = false"
          >
            <span class="lod-tile-level">LOD{{ index + 1 }}</span>
            <span v-if="lod === null" class="lod-tile-missing">{{
              $t("package.lodMissing")
            }}</span>
          </button>
        </div>
      </div>
    </div>
    <div v-if="lightPanelOpen" class="viewport-overlay viewport-light-panel">
      <label>
        <span>{{ $t("package.lightAzimuth") }}</span>
        <input v-model.number="lightAzimuth" type="range" min="0" max="360" />
      </label>
      <label>
        <span>{{ $t("package.lightElevation") }}</span>
        <input v-model.number="lightElevation" type="range" min="5" max="175" />
      </label>
      <label>
        <span>{{ $t("package.lightBrightness") }}</span>
        <input v-model.number="brightness" type="range" min="0" max="2" step="0.05" />
      </label>
    </div>
    <div v-if="infoPanelOpen" class="viewport-overlay info-panel">
      <div class="info-card" role="dialog" :aria-label="$t('package.modelInfo')">
        <header class="lod-card-header">
          <span>{{ $t("package.modelInfo") }}</span>
          <div class="info-header-actions">
            <button
              type="button"
              class="lod-close"
              :title="$t('package.copyDiagnostics')"
              :aria-label="$t('package.copyDiagnostics')"
              :disabled="!modelPayload?.diagnostics"
              @click="copyDiagnostics"
            >
              <FIcon :name="infoCopied ? 'Check' : 'Copy'" :size="13" aria-label="" />
            </button>
            <button
              type="button"
              class="lod-close"
              :aria-label="$t('common.close')"
              @click="infoPanelOpen = false"
            >
              <FIcon name="X" :size="13" aria-label="" />
            </button>
          </div>
        </header>
        <pre class="info-pre">{{ modelPayload?.diagnostics || $t("package.modelInfoEmpty") }}</pre>
      </div>
    </div>
    <div class="viewport-overlay viewport-visibility" role="group" :aria-label="$t('package.visibilityToggles')">
      <button
        v-for="name in GROUP_KEYS"
        :key="name"
        type="button"
        :class="{ off: groupVisibility[name] === false }"
        :aria-pressed="groupVisibility[name] !== false"
        :title="$t(`package.group${name[0].toUpperCase()}${name.slice(1)}`)"
        @click="$emit('toggle-layer', name)"
      >
        <FIcon :name="groupVisibility[name] === false ? 'EyeOff' : 'Eye'" :size="13" aria-label="" />
        <span>{{ $t(`package.group${name[0].toUpperCase()}${name.slice(1)}`) }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.viewport-pane {
  background: var(--surface-elevated);
  display: grid;
  min-height: 0;
  min-width: 0;
  position: relative;
}
.viewport-3d {
  cursor: grab;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  touch-action: none;
}
.viewport-3d:active {
  cursor: grabbing;
}
.viewport-3d canvas {
  display: block;
}
.viewport-overlay {
  position: absolute;
  z-index: 1;
}
.viewport-status {
  align-self: center;
  justify-self: center;
  background: color-mix(in srgb, var(--surface) 82%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: grid;
  font-size: 12px;
  padding: 8px 14px;
  pointer-events: none;
}
.viewport-tools {
  display: flex;
  gap: 4px;
  right: 10px;
  top: 10px;
}
.viewport-tools button {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 28px;
  min-width: 28px;
}
.viewport-tools button:hover,
.viewport-tools button.active {
  color: var(--foreground);
}
.viewport-tools button.active {
  background: var(--accent);
}
.viewport-light-panel {
  background: color-mix(in srgb, var(--surface) 92%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: grid;
  gap: 6px;
  padding: 10px;
  right: 10px;
  top: 48px;
}
.viewport-light-panel label {
  align-items: center;
  display: grid;
  font-size: 11px;
  grid-template-columns: auto 140px;
  gap: 8px;
}
.viewport-light-panel span {
  color: var(--subtle-foreground);
}
.viewport-light-panel input[type="range"] {
  accent-color: var(--primary);
  width: 140px;
}
.lod-mask {
  background: color-mix(in srgb, var(--surface) 55%, transparent);
  display: grid;
  inset: 0;
  place-items: center;
  z-index: 2;
}
.lod-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg, 0 8px 28px rgb(0 0 0 / 0.35));
  display: grid;
  gap: 12px;
  padding: 14px;
  width: min(340px, calc(100% - 32px));
}
.lod-card-header {
  align-items: center;
  color: var(--foreground);
  display: flex;
  font-size: 13px;
  font-weight: 650;
  justify-content: space-between;
}
.lod-close {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  min-height: 24px;
  min-width: 24px;
  justify-content: center;
}
.lod-close:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.lod-tiles {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(4, 1fr);
}
.lod-tile {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-md, var(--radius-sm));
  color: var(--foreground);
  cursor: pointer;
  display: grid;
  font: inherit;
  gap: 2px;
  justify-items: center;
  min-height: 52px;
  padding: 8px 4px;
  transition: border-color 120ms ease, box-shadow 120ms ease;
}
.lod-tile:hover:not(:disabled) {
  border-color: var(--accent);
}
.lod-tile.active {
  border-color: var(--accent);
  box-shadow:
    0 0 0 1px var(--accent),
    0 0 10px color-mix(in srgb, var(--accent) 45%, transparent);
}
.lod-tile:disabled {
  color: var(--subtle-foreground);
  cursor: not-allowed;
  opacity: 0.55;
}
.lod-tile-level {
  font-size: 12.5px;
  font-weight: 650;
}
.lod-tile-missing {
  color: var(--subtle-foreground);
  font-size: 10.5px;
}
.info-panel {
  right: 10px;
  top: 48px;
}
.info-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg, 0 8px 28px rgb(0 0 0 / 0.35));
  display: grid;
  gap: 8px;
  max-height: min(60vh, 520px);
  padding: 12px;
  width: min(540px, calc(100vw - 32px));
}
.info-header-actions {
  display: inline-flex;
  gap: 2px;
}
.info-pre {
  color: var(--foreground);
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
  overflow: auto;
  user-select: text;
  white-space: pre;
}
.viewport-visibility {
  display: grid;
  gap: 3px;
  left: 10px;
  top: 10px;
}
.viewport-visibility button {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 5px;
  justify-content: flex-start;
  min-height: 24px;
  padding: 0 8px;
}
.viewport-visibility button.off {
  color: var(--subtle-foreground);
  opacity: 0.7;
}
</style>
