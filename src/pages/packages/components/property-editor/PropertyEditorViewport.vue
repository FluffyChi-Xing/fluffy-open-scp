<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { ThreeViewer, disposeObject } from "@/lib/three-viewer";
import { parseLotModelObjects, pngBlobUrl } from "@/lib/three-gltf";
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
  lotMaskPng: string | null;
  selectedId: string | null;
  hiddenUnits: Set<string>;
  groupVisibility: Record<string, boolean>;
  modelState: ModelState;
  /** 通道实验开关（仅精细模式显示）；关闭时恒用 G（数据实测）。 */
  specExperiment?: boolean;
  /** true = G 通道（数据实测 specularity）；false = B 通道（源码字面）。 */
  specChannelG?: boolean;
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

/** 存活 tint 材质的 uSpecG uniform 引用（通道实验热切换，免重建）。 */
const specUniformRefs: { value: number }[] = [];
/** 实验关闭恒用 G（数据实测）；开启后按 G/B 切换观察。 */
const effectiveSpecG = () => (props.specExperiment && props.specChannelG === false ? 0 : 1);
watch([() => props.specExperiment, () => props.specChannelG], () => {
  const value = effectiveSpecG();
  for (const uniform of specUniformRefs) uniform.value = value;
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
 * TEXCOORD_2 = facade 世界投影 UV（Float4.xy），TEXCOORD_1.x = materialIndex。
 * fragment：baseUv = frac(vTintUv)*regionXform.xy + regionXform.zw → tint 查表
 * → palette 查色（palU=row0.x + tint.r×sub；palV=variation 行 0 + tint.g×sub）
 * ×(tint.b*2)，A<0.5 镂空 discard；法线图同 UV 重采样。
 * 5a 材质质感（源码 building4DeferredPS）：shaderMap.b×2=specStrength、
 * palette 色 a×(tint.b*2) 立方×2048+1=specE、palette surface 行（末行
 * kSurfacePalV）a×(tint.b*2)=reflectance、gloss=saturate(色a×specStrength)、
 * AO=normalMap.a；SimCityLighting 太阳 Blinn-Phong-Schlick 高光 +
 * EnvLighting 常数天空近似（gloss×0.75 能量劈分，经 tint 乘）。
 * 唯一偏离源码处：object 法线 Z<-0.3（下向面）豁免镂空——游戏镂空模板被
 * 地板/底面继承（共用 facade UV），仰视穿透是原版瑕疵、相机不可达故未处理
 * （docs/rendering.md §3、tint_underface_probe 取证：窗口内 36% 镂空）。
 */
function attachTintShader(
  material: ThreeNamespace.MeshStandardMaterial,
  uniforms: {
    tintMap: { value: ThreeNamespace.Texture };
    paletteMap: { value: ThreeNamespace.Texture };
    shaderMapMap: { value: ThreeNamespace.Texture | null };
    paramsMap: { value: ThreeNamespace.Texture | null };
    uParamCols: { value: number };
    uSunDir: { value: ThreeNamespace.Vector3 };
    uSunColor: { value: ThreeNamespace.Color };
    uSkyColor: { value: ThreeNamespace.Color };
    uSpecG: { value: number };
  },
  paramsReady: boolean,
  shaderMapReady: boolean,
) {
  // 注意：three 默认编译为 GLSL ES 1.00——texelFetch/ivec2 不可用，
  // 参数表用 texture2D + 预计算 V 寻址（Nearest 采样取整行）。
  material.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, uniforms);
    shader.vertexShader = shader.vertexShader
      .replace(
        "#include <common>",
        `#include <common>
attribute vec2 uv1;
attribute vec2 uv2;
uniform float uParamCols;
varying vec2 vTintUv;
varying float vMatU;
varying float vObjUp;`,
      )
      .replace(
        "#include <uv_vertex>",
        `#include <uv_vertex>
vTintUv = uv2;
vMatU = (uv1.x * 255.0 + 0.5) / uParamCols;`,
      )
      .replace(
        "#include <beginnormal_vertex>",
        `#include <beginnormal_vertex>
vObjUp = normalize(objectNormal).z;`,
      );
    shader.fragmentShader = shader.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
varying vec2 vTintUv;
varying float vMatU;
varying float vObjUp;
uniform sampler2D tintMap;
uniform sampler2D paletteMap;
uniform vec3 uSunDir;
uniform vec3 uSunColor;
uniform vec3 uSkyColor;
uniform float uSpecG;
#ifdef TINT_PARAMS
uniform sampler2D paramsMap;
#endif
#ifdef TINT_SHADERMAP
uniform sampler2D shaderMapMap;
#endif`,
      )
      .replace(
        "#include <map_fragment>",
        `#include <map_fragment>
#ifdef TINT_PARAMS
        vec4 xform = texture2D(paramsMap, vec2(vMatU, 0.375));
        vec4 palOrigin = texture2D(paramsMap, vec2(vMatU, 0.125));
#else
        vec4 xform = vec4(1.0, 1.0, 0.0, 0.0);
        vec4 palOrigin = vec4(0.0);
#endif
        vec2 tUv = fract(vTintUv) * xform.xy + xform.zw;
        vec4 tintValues = texture2D(tintMap, tUv);
        vec2 scSub = tintValues.rg * vec2(1.0 / 512.0, 1.0 / 16.0) + vec2(1.0 / 1024.0, 1.0 / 32.0);
        vec4 scPalColor = vec4(1.0);
        float scTintMul = tintValues.b * 2.0;
        float scExempt = 0.0;
        if (tintValues.a < 0.5) {
          if (vObjUp >= -0.3) discard;
          // 下向面豁免（观察器缓解）：游戏 building4Clip 的镂空模板被地板/
          // 底面继承（底面与立面共用 facade UV），从下仰视出现穿透洞——
          // 游戏相机不可达此视角故原版未处理。豁免片段跳过调色保持白模观感。
          scExempt = 1.0;
        } else {
          scPalColor = texture2D(paletteMap, vec2(palOrigin.x + scSub.x, scSub.y));
          diffuseColor.rgb *= scPalColor.rgb * scTintMul;
          #ifdef USE_NORMALMAP
          diffuseColor.rgb *= texture2D(normalMap, tUv).a; // artistAO
          #endif
        }
        // 5a spec 四标量（building4DeferredPS；色/表面行均 ×tintMul）。
        // specStrength 通道：源码读 .b，但资产实证 specularity 画在 .g
        // （玻璃楼 G=159-186 带对角高光笔触、B≈0-8；金样本窗洞 G=11；
        // SUGC PDF 的"B=Specularity"与数据不符）——uSpecG=1 走 G（默认），
        // 0 回溯源码 B。
        float scSpecStrength = 0.0;
        #ifdef TINT_SHADERMAP
        if (scExempt < 0.5) {
          vec4 scShader = texture2D(shaderMapMap, tUv);
          scSpecStrength = mix(scShader.b, scShader.g, uSpecG) * 2.0;
        }
        #endif
        float scSpecA = scPalColor.a * scTintMul;
        float scSpecE = scSpecA * scSpecA * scSpecA * 2048.0 + 1.0;
        float scGloss = clamp(scSpecA * scSpecStrength, 0.0, 1.0);
        vec4 scSurface = texture2D(paletteMap, vec2(palOrigin.x + scSub.x, 0.875 + scSub.y)); // kSurfacePalV
        float scReflectance = scSurface.a * scTintMul;`,
      )
      .replace(
        "#include <normal_fragment_maps>",
        `#include <normal_fragment_maps>
        #ifdef USE_NORMALMAP
        {
          vec2 nUv = fract(vTintUv) * xform.xy + xform.zw;
          mat3 tbn = getTangentFrame( - vViewPosition, nonPerturbedNormal, nUv );
          vec3 mapN = texture2D( normalMap, nUv ).xyz * 2.0 - 1.0;
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
        }`,
      );
  };
  material.customProgramCacheKey = () =>
    `building4-tint${shaderMapReady ? "+sm" : ""}${paramsReady ? "+pm" : ""}`;
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
      paramsTex: buildParamsTexture(THREE, material),
      paramCols: material.paramCols,
    })),
  );
  if (token !== rebuildToken) return;
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
        const uSpecGUniform = { value: effectiveSpecG() };
        specUniformRefs.push(uSpecGUniform);
        attachTintShader(
          tinted,
          {
            tintMap: { value: tint.tintTex },
            paletteMap: { value: tint.paletteTex },
            shaderMapMap: { value: tint.shaderTex },
            paramsMap: { value: tint.paramsTex },
            uParamCols: { value: tint.paramCols },
            // 5a：太阳/天空占位参数（游戏为日循环 cSunSkyInfo，观察器取固定
            // 晴天正午近似；three-world Y-up）
            uSunDir: { value: new THREE.Vector3(0.35, 0.8, 0.45).normalize() },
            uSunColor: { value: new THREE.Color(1.0, 0.95, 0.85) },
            uSkyColor: { value: new THREE.Color(0.3, 0.42, 0.55) },
            uSpecG: uSpecGUniform,
          },
          Boolean(tint.paramsTex),
          Boolean(tint.shaderTex),
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
    });
  }

  // Lot 地面矩形（LotSize）；有 LotMask 时异步贴四色量化图。
  if (props.lotSize) {
    const ground = buildLotRect(THREE, props.lotSize);
    // C# CreateLotModel：地面按 LotPlacementTransform 的逆矩阵摆放——
    // 建筑在地块内不居中时，逆变换把遮罩图案对回建筑原点。
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
      ground.matrixAutoUpdate = false;
      ground.matrix.copy(inverse);
    }
    instance.group("model").add(ground);
    if (props.lotMaskPng) {
      const generation = token;
      new THREE.TextureLoader().load(props.lotMaskPng, (texture) => {
        if (generation !== rebuildToken) {
          texture.dispose();
          return;
        }
        texture.colorSpace = THREE.SRGBColorSpace;
        const fill = ground.children.find((child) => (child as ThreeNamespace.Mesh).isMesh) as
          | ThreeNamespace.Mesh
          | undefined;
        if (fill) {
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
  applyBrightness();
  applyGroupVisibility();
  applyUnitVisibility();
  applySelection();
  sceneReady.value = true;
}

/** 日/夜亮度：仅缩放环境四灯；地块真实光源保持常亮（夜间灯依然亮）。 */
function applyBrightness() {
  viewer.value?.setEnvironmentBrightness(brightness.value);
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
