<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { disposeObject } from "@/lib/three-viewer";
import { parseLotModelObjects } from "@/lib/three-gltf";
import { renderTelemetry } from "@/lib/renderTelemetry";
import type { RenderTelemetryTrigger } from "@/lib/renderTelemetry";
import type * as ThreeNamespace from "three";
import type {
  DecalUnit,
  DecalUnitTexture,
  LotModelLodRef,
  LotModelPayload,
  LotUnitDto,
} from "@/api/tauri";
import type { ModelState, UnitGrouping } from "./usePropertyEditorSession";
import {
  buildPathLine,
  buildRealLightUnit,
  buildUnitObject,
  unitId,
  unitMatrix,
} from "./unitGizmos";
import { decalFrame, projectDecal } from "@/lib/decalProject";
import { useEditorViewport } from "./useEditorViewport";
import {
  applyDeferredMaterialMaps,
  applySunEnv,
  createSunEnv,
  dayFactor,
  loadTintTextures,
  makeTintMaterial,
  type SunEnvRefs,
} from "./refinedRender";
import { threeToRowMajor } from "./unitEditLayer";
import type { TransformControls } from "three/examples/jsm/controls/TransformControls.js";
import {
  applyGroundMask,
  buildLotRect,
  placementInverse,
} from "./editorGround";

export type EditorTool = "select" | "translate" | "rotate" | "scale";

/**
 * Lot 场景装配层（业务形态相关）：在 useEditorViewport 底座上组合
 * refinedRender（tint shader/日夜环境）、editorGround（地面）与
 * unitGizmos（六类 Unit）完成 SCP lot 视口。浮层 UI 亦在此层。
 * 底座/业务拆分见 docs/roadmap/asset-development.md §二（PE-重构-1）。
 */
const props = defineProps<{
  modelPayload: LotModelPayload | null;
  /** LOD1~LOD4 资源位置（index 0 = LOD1）；缺失级为 null。 */
  modelLods: (LotModelLodRef | null)[];
  activeLod: number;
  renderMode: "default" | "refined";
  grouping: UnitGrouping;
  lotSize: [number, number] | null;
  lotTilePeriod: [number, number] | null;
  /** LotPlacementTransform 行主序 12 floats；地面矩形取其逆对齐建筑。 */
  lotPlacement: number[] | null;
  /** LotColor1-4 RGBA（A = 地面贴图索引 0-15）。 */
  lotColors: [number, number, number, number][];
  /** LotColor1-4 是否实际存在（false = 回退色，不参与着色）。 */
  lotColorsAuthored: boolean[];
  /** LotBorderColor1-4 的 sRGB RGB（mask 渐变带描边色）。 */
  lotBorderColors: [number, number, number][];
  /** borderWidth1-4（边框带半宽）；全 0 = 无边框。 */
  lotBorderWidths: number[];
  /** LotOverlayBoxOffset：地面 quad 中心覆盖；null = 引擎回退锚点包围盒中心。 */
  lotOverlayBoxOffset: [number, number] | null;
  /** Model Bounding Box（0x00F9EFBA）的 xy 中心（模型空间）；null = 无属性。 */
  lotModelBBoxCenter: [number, number] | null;

  lotMaskPng: string | null;
  /** LotMask 原始通道权重图（v4 软混合输入）。 */
  /** LotMask 原始通道权重（未压缩 RGBA base64；A = LC4 权重）。 */
  lotMaskRawRgba: string | null;
  /** 默认模式地表反照率（通道平色+底图格；缺失时回退量化图）。 */
  lotAlbedoPng: string | null;
  /** 精细模式贴花纹理（按 decal 的 category+index 对应）。 */
  decalTextures: DecalUnitTexture[];
  /** "Lot Textures" 地表共享纹理（data URL；精细模式地面 v2 用）。 */
  lotSurfacePng: string | null;
  /** 全局共享染色图集（s10）data URL。 */
  lotTintAtlasPng: string | null;
  /** 全局共享法线图集（s15）data URL：地面 normalMap。 */
  lotNormalAtlasPng: string | null;
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
  /** 当前编辑工具（select = 仅拾取；其余挂 TransformControls 手柄）。 */
  tool?: EditorTool;
}>();
const emit = defineEmits<{
  select: [id: string | null];
  "toggle-layer": [name: string];
  "switch-lod": [index: number];
  "select-tool": [tool: EditorTool];
  /** 手柄拖拽结束提交变换（行主序 12 floats），由壳落本地编辑命令。 */
  "commit-transform": [id: string, matrix: number[]];
  /** 拖拽中的实时变换（id 为 null 表示结束）；坐标面板即时显示用。 */
  "live-transform": [
    id: string | null,
    value: {
      position: [number, number, number];
      rotation: [number, number, number];
      scale: [number, number, number];
    } | null,
  ];
}>();
useI18n();
const viewport = useEditorViewport({
  onTapUnit: (id) => emit("select", id),
});
const lightPanelOpen = ref(false);
const lodPanelOpen = ref(false);
const infoPanelOpen = ref(false);
const infoCopied = ref(false);
let infoCopiedTimer: ReturnType<typeof setTimeout> | undefined;
const lightAzimuth = ref(45);
const lightElevation = ref(55);
/** 日/夜模拟：全局环境亮度倍率（1 = 当前观感，0 ≈ 夜，2 = 正午）。 */
const brightness = ref(1);

watch([lightAzimuth, lightElevation], () => {
  viewport.viewer.value?.setKeyLight(lightAzimuth.value, lightElevation.value);
});
watch(brightness, () => applyBrightness());

/** 存活 tint 材质的 uSpecMode uniform 引用（通道实验热切换，免重建）。 */
const specUniformRefs: { value: number }[] = [];

let envRefs: SunEnvRefs | null = null;

const timeOfDay = () => props.timeOfDay ?? 12;

function applySun() {
  if (envRefs) applySunEnv(envRefs, timeOfDay(), props.powered);
}
watch([() => props.specExperiment, () => props.specMode], () => {
  for (const uniform of specUniformRefs) uniform.value = 2;
});
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

function kindGroup(kind: LotUnitDto["kind"]) {
  return kind === "pathPoint" ? "paths" : `${kind}s`;
}

/** TransformControls 手柄（PE-重构-3）：拖拽期间挂起轨道相机，
 * 抬手提交一次行主序矩阵命令。helper 加入 scene 根（相机空间朝向）。 */
let gizmo: TransformControls | null = null;
let dragStartMatrix: ThreeNamespace.Matrix4 | null = null;
/** 默认主灯方位只初始化一次（用户调节后 rebuild 不再重置）。 */
let keyLightInitialized = false;
/** 上一次 rebuild 的模型载荷（身份变化 = 模型/LOD 更换 → 重构图）。 */
let lastPayload: LotModelPayload | null | undefined;

function emitLiveTransform(object: ThreeNamespace.Object3D | null) {
  if (!object || typeof object.userData?.unitId !== "string") {
    emit("live-transform", null, null);
    return;
  }
  const THREE = viewport.viewer.value?.THREE;
  if (!THREE) return;
  const position = new THREE.Vector3();
  const quaternion = new THREE.Quaternion();
  const scale = new THREE.Vector3();
  new THREE.Matrix4()
    .compose(object.position, object.quaternion, object.scale)
    .decompose(position, quaternion, scale);
  const euler = new THREE.Euler().setFromQuaternion(quaternion, "XYZ");
  const deg = 180 / Math.PI;
  emit("live-transform", object.userData.unitId as string, {
    position: [position.x, position.y, position.z],
    rotation: [euler.x * deg, euler.y * deg, euler.z * deg],
    scale: [scale.x, scale.y, scale.z],
  });
}

async function ensureGizmo() {
  const instance = viewport.viewer.value;
  if (!instance || gizmo) return;
  const { TransformControls: Controls } =
    await import("three/examples/jsm/controls/TransformControls.js");
  if (viewport.viewer.value !== instance) return;
  const controls = new Controls(instance.camera, instance.domElement);
  controls.size = 0.85;
  controls.addEventListener("objectChange", () => {
    if (controls.dragging) emitLiveTransform(controls.object);
  });
  controls.addEventListener("dragging-changed", (event) => {
    const dragging = (event as unknown as { value: boolean }).value;
    instance.setOrbitEnabled(!dragging);
    if (dragging) {
      const object = controls.object;
      dragStartMatrix = object
        ? new instance.THREE.Matrix4().compose(
            object.position,
            object.quaternion,
            object.scale,
          )
        : null;
    } else {
      commitGizmo(controls);
      dragStartMatrix = null;
      emit("live-transform", null, null);
    }
  });
  instance.scene.add(controls.getHelper());
  gizmo = controls;
  updateGizmo();
}

function commitGizmo(controls: TransformControls) {
  const object = controls.object;
  if (!object || typeof object.userData?.unitId !== "string") return;
  const THREE = viewport.viewer.value?.THREE;
  if (!THREE) return;
  const matrix = new THREE.Matrix4().compose(
    object.position,
    object.quaternion,
    object.scale,
  );
  // 未产生位移的点击（拖拽起止矩阵相同）不产生冗余命令
  if (dragStartMatrix && matrix.equals(dragStartMatrix)) return;
  emit(
    "commit-transform",
    object.userData.unitId as string,
    threeToRowMajor(matrix),
  );
}

function updateGizmo() {
  const controls = gizmo;
  if (!controls) return;
  controls.detach();
  const tool = props.tool ?? "select";
  if (tool === "select" || !props.selectedId) return;
  const object = viewport.unitObjects.get(props.selectedId);
  if (!object) return;
  controls.setMode(tool);
  controls.attach(object);
}

/** 本次重建的触发来源，供渲染遥测标注（在 watcher 里按变化项判定）。 */
let pendingTrigger: RenderTelemetryTrigger = "first_load";

/** 贴花投影命中/回退计数（每次装配前重置），供 decal_render 遥测。 */
const decalStats = { projected: 0, fallback: 0 };

/** rebuild 包装：模型载荷身份变化时重新构图（编辑操作保持镜头）。 */
function rebuildScene() {
  const reframe = props.modelPayload !== lastPayload;
  lastPayload = props.modelPayload;
  // 只改 trigger：sessionKey 已由 usePropertyEditorSession 设好。
  renderTelemetry.setTrigger(pendingTrigger);
  return viewport.rebuild(assembleScene, { reframe });
}

/** 量化 mask 图的尺寸（raw RGBA 字节流构造 ImageData 时需要宽高）。 */
async function loadMaskImageDims(): Promise<{
  width: number;
  height: number;
} | null> {
  const url = props.lotMaskPng ?? props.lotAlbedoPng;
  if (!url) return null;
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const element = new Image();
      element.onload = () => resolve(element);
      element.onerror = () => reject(new Error("lot mask failed"));
      element.src = url;
    });
    return { width: image.width, height: image.height };
  } catch {
    return null;
  }
}

/** LotMask 原始通道权重图 → ImageData（compose 输入）。
 *  后端是未压缩 RGBA 字节流 base64（A = LC4 权重），**必须用 atob 直接构造
 *  ImageData**——若经 canvas 解码，预乘 alpha 会按 LC4 权重等比压缩/清零
 *  LC1-3 权重，精细合成随即满地判为草皮（2026-09-13 消防局对拍根因）。 */
function loadRawMaskPixels(width: number, height: number): ImageData | null {
  const base64 = props.lotMaskRawRgba;
  if (!base64) return null;
  const binary = atob(base64);
  const expected = width * height * 4;
  if (binary.length < expected) return null;
  const pixels = new Uint8ClampedArray(expected);
  for (let index = 0; index < expected; index += 1) {
    pixels[index] = binary.charCodeAt(index);
  }
  return new ImageData(pixels, width, height);
}

/** "Lot Textures" 地表纹理 → 像素数据（compose v2 输入）。 */
async function loadImageDataFromUrl(
  url: string | null,
): Promise<ImageData | null> {
  if (!url) return null;
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const element = new Image();
      element.onload = () => resolve(element);
      element.onerror = () => reject(new Error("image failed"));
      element.src = url;
    });
    const canvas = document.createElement("canvas");
    canvas.width = image.width;
    canvas.height = image.height;
    const context = canvas.getContext("2d");
    if (!context) return null;
    context.drawImage(image, 0, 0);
    return context.getImageData(0, 0, image.width, image.height);
  } catch {
    return null;
  }
}

async function loadSurfacePixels(): Promise<ImageData | null> {
  const url = props.lotSurfacePng;
  if (!url) return null;
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const element = new Image();
      element.onload = () => resolve(element);
      element.onerror = () => reject(new Error("lot surface failed"));
      element.src = url;
    });
    const canvas = document.createElement("canvas");
    canvas.width = image.width;
    canvas.height = image.height;
    const context = canvas.getContext("2d");
    if (!context) return null;
    context.drawImage(image, 0, 0);
    return context.getImageData(0, 0, image.width, image.height);
  } catch {
    return null;
  }
}

onMounted(async () => {
  await viewport.ready;
  await rebuildScene();
  await ensureGizmo();
});
onBeforeUnmount(() => {
  gizmo?.detach();
  gizmo?.dispose();
  gizmo = null;
});

/** 业务场景装配：模型材质 → 地面 → 六类 Unit → 路径折线。 */
async function assembleScene(
  ctx: Parameters<Parameters<typeof viewport.rebuild>[0]>[0],
) {
  const { viewer: instance, THREE } = ctx;
  specUniformRefs.length = 0;

  const payload = props.modelPayload;
  const modelObjects = payload ? await parseLotModelObjects(payload.glbs) : [];
  if (ctx.isStale()) {
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
  /** 贴花投影目标：全部建筑网格（在 lot 局部空间为单位变换）。 */
  const buildingMeshes: ThreeNamespace.Mesh[] = [];
  const tintSpan = renderTelemetry.begin("texture_compose", {
    phase: "tint",
    refined: props.renderMode === "refined",
  });
  const tintResolved =
    props.renderMode === "refined" && payload
      ? await loadTintTextures(
          THREE,
          payload.materials ?? [],
          ctx.registerTextureUrl,
          ctx.maxAnisotropy,
        )
      : [];
  tintSpan.end({
    materials: payload?.materials?.length ?? 0,
    tinted: tintResolved.filter((entry) => entry?.tintTex).length,
  });
  if (ctx.isStale()) return;
  // 5d 日/夜环境共享 uniform（全部 tint 材质引用同一组对象）。
  // 必须立刻按当前时段求值：天空三段色与 uSkyLumRef（球面均值）都依赖它，
  // 否则首帧用的是 createSunEnv 的占位值（间接光会整体偏暗）。
  const env = createSunEnv(THREE);
  applySunEnv(env, timeOfDay(), props.powered);
  envRefs = env;
  // 注：空腔质心锚定已被统计检验否定（lot_cavity_stats 400 样本，
  // d0-d1 配对 t=-5.15：bbox 中心到空腔质心反而更远）——建筑保持
  // 居中（bbox≈0 实证），mask 空腔与其错位另有机制（0x0CCB7FD2/D3
  // UV 字段为头号嫌疑，待取证）。
  for (const [index, object] of modelObjects.entries()) {
    const materialIndex = payload?.meshMaterialIndices[index] ?? 0;
    const uvKind = payload?.meshUvKinds[index] ?? 0;
    object.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (!mesh.isMesh) return;
      // 贴花投影的候选面（精细模式才用；建筑网格在 lot 局部空间为单位变换）
      buildingMeshes.push(mesh);
      if (props.renderMode !== "refined") {
        mesh.material = whiteMaterial;
        return;
      }
      const tint = tintResolved[materialIndex];
      if (uvKind === 2 && tint?.tintTex && tint.paletteTex) {
        // facade tint 着色器：逐像素复刻 building4 链（tint 查表 → palette 查色）
        const [tinted, specUniform] = makeTintMaterial(THREE, tint, env);
        specUniformRefs.push(specUniform);
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
  if (props.renderMode === "refined" && payload) {
    const deferredSpan = renderTelemetry.begin("texture_compose", {
      phase: "deferred",
    });
    applyDeferredMaterialMaps(
      THREE,
      payload,
      materialGroups,
      ctx.registerTextureUrl,
      ctx.isStale,
      ctx.maxAnisotropy,
    );
    deferredSpan.end({ groups: materialGroups.length });
  }

  // Lot 地面矩形（LotSize）；有 LotMask 时异步贴四色量化图。
  if (props.lotSize) {
    const groundSpan = renderTelemetry.begin("lot_render", {
      refined: props.renderMode === "refined",
    });
    const ground = buildLotRect(THREE, props.lotSize);
    // 关闭自动更新：矩阵完全由 placement 逆决定，防止渲染循环覆盖。
    if (props.lotPlacement) {
      ground.matrix.copy(placementInverse(THREE, props.lotPlacement));
    }
    // 引擎定位定案（FUN_008ba1c0/FUN_007e2260 逐字 + 五样本对照，2026-09-19）：
    // 地面 quad（LotSize 尺寸）中心 = placement 变换后的 Model Bounding Box
    // 中心（0x00F9EFBA）；LotOverlayBoxOffset（0x0CCB7FC9）存在时覆盖之。
    // 建筑在编辑器中以模型原点摆放，故地面相对建筑 = R·center（placement
    // 平移 t 属整组装位，不进入相对关系）。五样本对照：塔楼 ≈0、图书馆
    // −0.41、EP1 房 +2.89、消防局 0（相对）——旧实现误差 = bboxC + 2·t
    // （消防局 8m = 2×4m 平移，用户目视的大偏移）。
    const pm = props.lotPlacement;
    const r00 = pm ? pm[0] : 1;
    const r01 = pm ? pm[3] : 0;
    const r10 = pm ? pm[1] : 0;
    const r11 = pm ? pm[4] : 1;
    const centerLocal =
      props.lotOverlayBoxOffset ?? props.lotModelBBoxCenter ?? [0, 0];
    const tx = r00 * centerLocal[0] + r01 * centerLocal[1];
    const ty = r10 * centerLocal[0] + r11 * centerLocal[1];
    if (Math.abs(tx) > 1e-4 || Math.abs(ty) > 1e-4) {
      ground.matrix.premultiply(
        new THREE.Matrix4().makeTranslation(tx, ty, 0),
      );
    }
    ground.matrixAutoUpdate = false;
    // 独立 lot 组：与建筑模型分开控制可见性（关模型不连地面一起隐藏，
    // 2026-09-13 用户对拍需求）。
    instance.group("lot").add(ground);
    if (props.lotMaskPng || props.lotAlbedoPng) {
      // v2：先加载地表纹理像素，失败/缺失时 compose 回退 v1
      const [surface, maskDims, tintAtlas, normalAtlas] = await Promise.all([
        loadSurfacePixels(),
        loadMaskImageDims(),
        loadImageDataFromUrl(props.lotTintAtlasPng),
        loadImageDataFromUrl(props.lotNormalAtlasPng),
      ]);
      if (!surface) {
        // 精细渲染的材质替换依赖真实图集；静默回退占位 tile 会把沥青画成
        // 亮灰（2026-09-13 对拍教训），必须让用户看到原因。
        console.warn(
          "[lot-ground] 'Lot Textures' surface unavailable — refined ground will use placeholder tiles. Open SimCity_Graphics.package (and the lot's own package) for the real atlas.",
        );
      }
      const rawMask = maskDims
        ? loadRawMaskPixels(maskDims.width, maskDims.height)
        : null;
      applyGroundMask({
        rawMask,
        surface,
        tintAtlas,
        normalAtlas,
        THREE,
        ground,
        maskPng: props.lotMaskPng,
        albedoPng: props.lotAlbedoPng,
        lotSize: props.lotSize,
        tilePeriod: props.lotTilePeriod,
        refined: props.renderMode === "refined",
        lotColors: props.lotColors,
        lotColorsAuthored: props.lotColorsAuthored,
        lotBorderColors: props.lotBorderColors,
        lotBorderWidths: props.lotBorderWidths,
        lotOverlayBoxOffset: props.lotOverlayBoxOffset,
        isStale: ctx.isStale,
      });
    }
    groundSpan.end({ masked: Boolean(props.lotMaskPng || props.lotAlbedoPng) });
  }

  /** 贴花材质：四色解码贴图 + 二值 alpha。投影片与浮空回退共用。 */
  function buildDecalMaterial(
    THREE: typeof ThreeNamespace,
    texture: DecalUnitTexture,
  ): ThreeNamespace.MeshBasicMaterial {
    const map = new THREE.TextureLoader().load(
      `data:image/png;base64,${texture.png}`,
    );
    map.colorSpace = THREE.SRGBColorSpace;
    return new THREE.MeshBasicMaterial({
      map,
      side: THREE.DoubleSide,
      // 四色解码对「四通道全 <128」的像素输出 alpha=0（原 SCP
      // RasterImage.CreateFromStream 同口径）——不理会 alpha 会把这些像素
      // 的 RGB=(0,0,0) 直接画成黑底。alpha 是二值的，alphaTest 即足够
      //（同地面 fill 口径），无需 transparent 的排序开销。
      alphaTest: 1 / 255,
      transparent: false,
      // 投影贴花与墙面共面，必须靠 polygonOffset 压过 z-fighting
      polygonOffset: true,
      polygonOffsetFactor: -4,
      polygonOffsetUnits: -4,
    });
  }

  /** 取贴花在 lot 局部的变换矩阵（无变换时为 None）。 */
  function applyDecalTransform(
    THREE: typeof ThreeNamespace,
    unit: DecalUnit,
    object: ThreeNamespace.Object3D,
  ) {
    if (!unit.transform) return;
    const matrix = unitMatrix(THREE, unit.transform);
    matrix.decompose(object.position, object.quaternion, object.scale);
  }

  /**
   * 浮空 quad 回退：投影落空（建筑未加载 / 贴花不属于任何建筑面）时仍让
   * 用户看得到、点得到该 decal。尺寸 = 2×scale × (2×scale)/aspect。
   */
  function buildDecalQuadFallback(
    THREE: typeof ThreeNamespace,
    unit: DecalUnit,
    texture: DecalUnitTexture,
  ): ThreeNamespace.Mesh {
    const aspect =
      texture.aspectRatio && texture.aspectRatio > 0 ? texture.aspectRatio : 1;
    // Scale 是半宽（原 SCP `UnitDecal.CreateGeometry`：`rectangle.Length = 2 * Scale`）。
    const width = Math.max((unit.scale ?? 4) * 2, 0.05);
    const height = Math.max(width / aspect, 0.05);
    const geometry = new THREE.PlaneGeometry(width, height);
    // U 轴镜像：引擎 decal PS 的 UV 是 `textureFloatPosition.xy * -0.5 + 0.5`
    // （U 取负，被 texXform 的 2 倍缩放补回量程），不翻会得到镜像文字
    //（用户实测 "Michael's CASINO" 左右反）。V 不翻（D3D v=0 在顶 +
    // 我们的 flipY=true 已抵消）。
    const uv = geometry.attributes.uv;
    for (let i = 0; i < uv.count; i += 1) uv.setX(i, 1 - uv.getX(i));
    uv.needsUpdate = true;
    const mesh = new THREE.Mesh(geometry, buildDecalMaterial(THREE, texture));
    // 回退仍按旧口径沿局部 -Z 让开 depth：引擎 `decalMaterialInfoWithObjectData`
    // 的 VS 取 -z（`float4(-z/-x/-y, 0)`）。
    mesh.translateZ(-(unit.depth ?? 0));
    return mesh;
  }

  /**
   * 精细模式贴花：优先按引擎 `decalProject` 的方式**投影到建筑几何**
   * （盒体积裁剪 + 盒内归一化 UV），失败则回退浮空 quad。
   *
   * 返回的顶层对象是**位于贴花原点的 Group**，使 TransformControls 挂在原点、
   * `unitObjects` 选中与 `userData.unitId` 注册照旧；投影几何子节点用
   * 逆矩阵抵消父变换，因此几何本身保持 lot 局部坐标。
   */
  async function buildDecalObject(
    THREE: typeof ThreeNamespace,
    unit: DecalUnit,
    texture: DecalUnitTexture,
    meshes: ThreeNamespace.Mesh[],
  ): Promise<ThreeNamespace.Object3D | null> {
    if (!texture.png) return null;
    const aspect =
      texture.aspectRatio && texture.aspectRatio > 0 ? texture.aspectRatio : 1;
    const frame = decalFrame(THREE, unit, aspect);
    const group = new THREE.Group();
    applyDecalTransform(THREE, unit, group);

    if (frame) {
      const geometry = await projectDecal(THREE, frame, meshes, unit.depth);
      if (geometry) {
        const mesh = new THREE.Mesh(
          geometry,
          buildDecalMaterial(THREE, texture),
        );
        const inverse = frame.matrix.clone().invert();
        mesh.matrixAutoUpdate = false;
        mesh.matrix.copy(inverse);
        group.add(mesh);
        decalStats.projected += 1;
        return group;
      }
    }
    // 回退：投影无命中（或缺少 scale/transform）时保留浮空 quad
    decalStats.fallback += 1;
    console.info(
      `[decal] ${unitId(unit)} 投影未命中建筑面，回退浮空 quad（可能在游戏的高细节 LOD 上）`,
    );
    group.add(buildDecalQuadFallback(THREE, unit, texture));
    return group;
  }

  const units: LotUnitDto[] = [
    ...props.grouping.lights,
    ...props.grouping.decals,
    ...props.grouping.props,
    ...props.grouping.effects,
    ...props.grouping.spawners,
    ...props.grouping.pathPoints,
  ];
  // 精细模式贴花：category+index → 解码纹理（后端已按 ID 查 atlas 条目）。
  const decalTextureByKey = new Map(
    props.decalTextures.map((texture) => [
      `${texture.category}:${texture.index}`,
      texture,
    ]),
  );
  const decalSpan = renderTelemetry.begin("decal_render", {
    decals: props.grouping.decals.length,
  });
  decalStats.projected = 0;
  decalStats.fallback = 0;
  for (const unit of units) {
    // 精细模式：光源用真实 three.js 光源、贴花投影到建筑面；其余组件保持标记锥
    const decalTexture =
      props.renderMode === "refined" && unit.kind === "decal"
        ? decalTextureByKey.get(`${unit.category}:${unit.index}`)
        : undefined;
    let object: ThreeNamespace.Object3D | null;
    if (props.renderMode === "refined" && unit.kind === "light") {
      object = buildRealLightUnit(THREE, unit);
    } else if (
      props.renderMode === "refined" &&
      unit.kind === "decal" &&
      decalTexture
    ) {
      // 贴图解码失败（无 png）→ 退回 gizmo，保证仍可见可选
      object =
        (await buildDecalObject(THREE, unit, decalTexture, buildingMeshes)) ??
        buildUnitObject(THREE, unit);
    } else {
      object = buildUnitObject(THREE, unit);
    }
    if (!object) continue;
    // 统一在此登记：精细模式的光源/贴花不再走 buildUnitObject，其 userData
    // 由此补齐（此前精细模式光源因此点不中）。
    object.userData.unitId = unitId(unit);
    object.userData.unitKind = unit.kind;
    instance.group(kindGroup(unit.kind)).add(object);
    ctx.unitObjects.set(unitId(unit), object);
    if (ctx.isStale()) return;
  }
  decalSpan.end({
    projected: decalStats.projected,
    fallback: decalStats.fallback,
    units: units.length,
  });

  // 路径折线：按 point_index 排序连接（pathPairs 语义未定，先 best-effort）。
  const points = [...props.grouping.pathPoints]
    .filter((point) => point.point)
    .sort((a, b) => (a.pointIndex ?? a.index) - (b.pointIndex ?? b.index))
    .map((point) => new THREE.Vector3(...point.point!));
  const line = buildPathLine(THREE, points);
  if (line) instance.group("paths").add(line);

  if (!keyLightInitialized) {
    instance.setKeyLight(45, 55);
    keyLightInitialized = true;
  }
  applySun();
  applyBrightness();
  viewport.applyGroupVisibility(props.groupVisibility);
  viewport.applyUnitVisibility(props.hiddenUnits);
  viewport.applySelection(props.selectedId);
}

/** 日/夜亮度：仅缩放环境四灯；地块真实光源保持常亮（夜间灯依然亮）。 */
function applyBrightness() {
  // 5d：夜间环境光随白昼因子压暗（亮度滑杆仍是用户侧总倍率）
  viewport.viewer.value?.setEnvironmentBrightness(
    brightness.value * (0.22 + 0.78 * dayFactor(timeOfDay())),
  );
}

defineExpose({
  captureRender: (options?: { includeDecals?: boolean }) =>
    viewport.captureRender(options),
});

watch(
  () => [props.modelPayload, props.renderMode, props.grouping] as const,
  ([payload, mode, grouping], previous) => {
    // 按变化项判定触发来源（首次拿到 payload 记 first_load，换级记 lod_switch）。
    if (payload !== previous?.[0]) {
      pendingTrigger = previous?.[0] == null ? "first_load" : "lod_switch";
    } else if (mode !== previous?.[1]) {
      pendingTrigger = "render_mode";
    } else if (grouping !== previous?.[2]) {
      pendingTrigger = "grouping";
    } else {
      pendingTrigger = "scene_rebuild";
    }
    void rebuildScene();
  },
);
watch(
  () => props.groupVisibility,
  () => viewport.applyGroupVisibility(props.groupVisibility),
  { deep: true },
);
watch(
  () => props.hiddenUnits,
  () => viewport.applyUnitVisibility(props.hiddenUnits),
);
watch(
  () => props.selectedId,
  () => viewport.applySelection(props.selectedId),
);
// 工具/选中/rebuild 代数变化 → 重挂或摘除手柄（选中对象会被重建）
watch([() => props.tool, () => props.selectedId, viewport.revision], () =>
  updateGizmo(),
);
</script>

<template>
  <div class="viewport-pane">
    <div
      :ref="(el) => (viewport.container.value = el as HTMLElement | null)"
      class="viewport-3d"
    />
    <div
      v-if="modelState === 'loading'"
      class="viewport-overlay viewport-status"
    >
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
        @click="viewport.resetView()"
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
            @click="
              emit('switch-lod', index);
              lodPanelOpen = false;
            "
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
        <input
          v-model.number="brightness"
          type="range"
          min="0"
          max="2"
          step="0.05"
        />
      </label>
    </div>
    <div v-if="infoPanelOpen" class="viewport-overlay info-panel">
      <div
        class="info-card"
        role="dialog"
        :aria-label="$t('package.modelInfo')"
      >
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
              <FIcon
                :name="infoCopied ? 'Check' : 'Copy'"
                :size="13"
                aria-label=""
              />
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
        <pre class="info-pre">{{
          modelPayload?.diagnostics || $t("package.modelInfoEmpty")
        }}</pre>
      </div>
    </div>
    <div
      class="viewport-overlay viewport-toolrail"
      role="group"
      :aria-label="$t('package.editorTools')"
    >
      <button
        v-for="entry in [
          { tool: 'select', icon: 'MousePointer', label: 'package.toolSelect' },
          { tool: 'translate', icon: 'Move', label: 'package.toolMove' },
          { tool: 'rotate', icon: 'RotateCw', label: 'package.toolRotate' },
          { tool: 'scale', icon: 'Expand', label: 'package.toolScale' },
        ]"
        :key="entry.tool"
        type="button"
        :class="{ active: (tool ?? 'select') === entry.tool }"
        :aria-pressed="(tool ?? 'select') === entry.tool"
        :title="`${$t(entry.label)} (${entry.tool === 'translate' ? 'G' : entry.tool === 'rotate' ? 'R' : entry.tool === 'scale' ? 'S' : 'Esc'})`"
        @click="emit('select-tool', entry.tool as EditorTool)"
      >
        <FIcon :name="entry.icon" :size="14" aria-label="" />
      </button>
    </div>
    <div
      class="viewport-overlay viewport-visibility"
      role="group"
      :aria-label="$t('package.visibilityToggles')"
    >
      <button
        v-for="name in [
          'model',
          'lot',
          'lights',
          'props',
          'decals',
          'effects',
          'spawners',
          'paths',
        ]"
        :key="name"
        type="button"
        :class="{ off: groupVisibility[name] === false }"
        :aria-pressed="groupVisibility[name] !== false"
        :title="$t(`package.group${name[0].toUpperCase()}${name.slice(1)}`)"
        @click="$emit('toggle-layer', name)"
      >
        <FIcon
          :name="groupVisibility[name] === false ? 'EyeOff' : 'Eye'"
          :size="13"
          aria-label=""
        />
        <span>{{
          $t(`package.group${name[0].toUpperCase()}${name.slice(1)}`)
        }}</span>
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
  transition:
    border-color 120ms ease,
    box-shadow 120ms ease;
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
.viewport-toolrail {
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  gap: 2px;
  left: 10px;
  padding: 3px;
  top: 50%;
  transform: translateY(-50%);
}
.viewport-toolrail button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 28px;
  min-width: 28px;
}
.viewport-toolrail button:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.viewport-toolrail button.active {
  background: var(--accent);
  color: var(--foreground);
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
