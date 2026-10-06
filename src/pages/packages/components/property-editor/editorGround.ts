import { composeRefinedGround } from "./refinedGround";
import { renderTelemetry } from "@/lib/renderTelemetry";
import type * as ThreeNamespace from "three";

/**
 * Lot 地面装配模块（scene contributor 的一部分）：LotSize 地面矩形、
 * LotPlacementTransform 逆矩阵摆放、LotMask 四色量化贴图。
 * 从 PropertyEditorViewport 原地抽出（PE-重构-1），逻辑未变。
 */

/** Lot 地面矩形：XY 平面（Z-up 贴地面）细边框 + 半透明填充。 */
export function buildLotRect(
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
      // 单面：消除从下方观察时的"正反面"镜像观感（对拍结论 2026-09-12）
      side: THREE.FrontSide,
      // 引擎 preview PS 有 clip(alpha - 1/255)：近零覆盖像素镂空
      alphaTest: 1 / 255,
    }),
  );
  // 方向定论（2026-09-12 mask 可视化 + 用户对拍）：identity 即正确——
  // "默认渲染道路正常"直接证明 mask UV 无需任何翻转；此前的"镜像感"
  // 实为 placement=None 时建筑居中 vs mask 足迹凹口偏置的错位
  // （maskAnchorOffset 自动锚定解决），不是镜像。
  fill.position.z = 0.02;
  group.add(border, fill);
  return group;
}

/**
 * LotPlacementTransform（行主序 12 floats）→ three 列主序逆矩阵。
 * C# CreateLotModel：地面按逆矩阵摆放——建筑在地块内不居中时，逆变换把
 * 遮罩图案对回建筑原点（mask 行列轴与模型 XY 轴直接对应）。
 * 注意：C# 在此还叠加了 R(−90°Z)，但那是 WPF/Helix 视口约定的补偿——
 * 真实数据检验（lot_anchor_probe 轴长统计：无 placement lot 741:48 支持
 * 0°；0x4EF6F6CD 视觉实证）表明引擎约定不旋转，旋转会导致 mask 相对
 * 建筑转置（用户实测 2026-09-10）。
 */
export function placementInverse(
  THREE: typeof ThreeNamespace,
  lotPlacement: number[],
): ThreeNamespace.Matrix4 {
  const m = lotPlacement;
  return new THREE.Matrix4()
    .set(
      m[0],
      m[3],
      m[6],
      m[9],
      m[1],
      m[4],
      m[7],
      m[10],
      m[2],
      m[5],
      m[8],
      m[11],
      0,
      0,
      0,
      1,
    )
    .invert();
}

/** 地面填充 mesh（buildLotRect 的 fill 子节点）。 */
export function groundFillMesh(
  ground: ThreeNamespace.Object3D,
): ThreeNamespace.Mesh | undefined {
  return ground.children.find(
    (child) => (child as ThreeNamespace.Mesh).isMesh,
  ) as ThreeNamespace.Mesh | undefined;
}

/** 尺度标注的一条锚点：轴别 + lot 局部系锚点（线段中点）+ 米数文本。 */
export interface LotDimensionAnchor {
  axis: "length" | "width" | "height";
  /** 线段中点（lot 局部系，Z-up）；调用方按地面矩阵换算到游戏系投影。 */
  point: [number, number, number];
  /** 显示文本，如 "32.0 m"（不含轴名前缀，前缀由 i18n 提供）。 */
  text: string;
}

/**
 * Lot 三维尺度标注（用户需求 2026-10-06）：自地块一角出发的工程标注式
 * 三线——长（X 边外平移）/宽（Y 边外平移）/高（竖直线，建筑实际高度，
 * 0 = 无建筑省略）。线段单 geometry LineSegments + 端部垂足刻线；文字
 * 标签由装配层按 anchors 逐帧投影（同 pick-tag 通道）。
 * 返回 null = 无可标注（缺 LotSize）。
 */
export function buildLotDimensions(
  THREE: typeof ThreeNamespace,
  lotSize: [number, number],
  height: number,
): { object: ThreeNamespace.LineSegments; anchors: LotDimensionAnchor[] } | null {
  const [w, d] = lotSize;
  if (!(w > 0) || !(d > 0)) return null;
  // 外偏移：随地块取寸（小 lot 不挤、大 lot 不远），高度线占角。
  const offset = Math.min(Math.max(Math.min(w, d) * 0.08, 0.5), 4);
  const z = 0.15;
  const x0 = -w / 2;
  const x1 = w / 2;
  const y0 = -d / 2;
  const y1 = d / 2;
  const xa = x0 - offset; // 宽/高线的标注轴
  const ya = y0 - offset; // 长线的标注轴
  const tick = offset * 0.35;
  const points: number[] = [];
  const push = (
    a: [number, number, number],
    b: [number, number, number],
  ) => {
    points.push(...a, ...b);
  };
  // 长：沿 X 边外侧的标注线 + 两端自地块角点的垂足延长刻线
  push([x0, ya, z], [x1, ya, z]);
  push([x0, y0, z], [x0, ya - tick, z]);
  push([x1, y0, z], [x1, ya - tick, z]);
  // 宽：沿 Y 边外侧
  push([xa, y0, z], [xa, y1, z]);
  push([x0, y0, z], [xa - tick, y0, z]);
  push([x0, y1, z], [xa - tick, y1, z]);
  // 高：占角竖直线 + 上下端横刻线
  const hasHeight = height > 0.01;
  if (hasHeight) {
    push([xa, ya, z], [xa, ya, height]);
    push([xa - tick, ya, z], [xa + tick, ya, z]);
    push([xa - tick, ya, height], [xa + tick, ya, height]);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute(
    "position",
    new THREE.Float32BufferAttribute(points, 3),
  );
  const object = new THREE.LineSegments(
    geometry,
    new THREE.LineBasicMaterial({ color: 0x0878fe }),
  );
  object.name = "__lotDimensions";
  const anchors: LotDimensionAnchor[] = [
    { axis: "length", point: [0, ya, z], text: `${w.toFixed(1)} m` },
    { axis: "width", point: [xa, 0, z], text: `${d.toFixed(1)} m` },
  ];
  if (hasHeight) {
    anchors.push({
      axis: "height",
      point: [xa, ya, height / 2],
      text: `${height.toFixed(1)} m`,
    });
  }
  return { object, anchors };
}

/**
 * 地面合成/贴图缓存（会话级）：compose 输入在编辑操作（transform/undo 引发的
 * grouping 全量重建）中完全不变，命中缓存 = 零合成零解码。key 由全部输入的
 * 身份（源字符串/数值）构成；容量 4 环形淘汰，淘汰时 dispose 贴图。
 */
const groundTextureCache = new Map<string, ThreeNamespace.Texture>();
const groundComposeCache = new Map<string, ThreeNamespace.Texture>();
// 4096² 贴图 64MB/张——容量 2 防显存失控（典型会话 1-2 个 lot）。
const GROUND_CACHE_CAP = 2;

function putGroundCache(key: string, texture: ThreeNamespace.Texture) {
  if (groundComposeCache.has(key)) return;
  groundComposeCache.set(key, texture);
  if (groundComposeCache.size > GROUND_CACHE_CAP) {
    const oldest = groundComposeCache.keys().next().value as string | undefined;
    if (oldest !== undefined) {
      groundComposeCache.get(oldest)?.dispose();
      groundComposeCache.delete(oldest);
    }
  }
}

/** 视口销毁/会话更换时清缓存（dispose 全部贴图）。 */
export function releaseGroundComposeCache(): void {
  for (const texture of groundComposeCache.values()) texture.dispose();
  groundComposeCache.clear();
  for (const texture of groundTextureCache.values()) texture.dispose();
  groundTextureCache.clear();
}

function loadGroundTexture(THREE: typeof ThreeNamespace, url: string): Promise<ThreeNamespace.Texture> {
  const cached = groundTextureCache.get(url);
  if (cached) return Promise.resolve(cached);
  return new THREE.TextureLoader().loadAsync(url).then((texture) => {
    groundTextureCache.set(url, texture);
    return texture;
  });
}

/**
 * 地面贴图 key：构成精细合成的全部输入身份（缓存正确性关键——任何一项
 * 变化都必须 miss）。
 */
function groundComposeKey(options: {
  maskPng: string | null;
  rawMaskKey: string | null;
  surfaceKey: string | null;
  normalAtlasKey: string | null;
  lotColors: [number, number, number, number][];
  lotBorderColors?: [number, number, number][];
  lotBorderPatternIndices?: number[];
  lotBorderWidths?: number[];
  lotSize?: [number, number] | null;
  tilePeriod?: [number, number] | null;
  baseTileIndex: number;
}): string {
  const o = options;
  return JSON.stringify([
    o.maskPng,
    o.rawMaskKey,
    o.surfaceKey,
    o.normalAtlasKey,
    o.lotColors,
    o.lotBorderColors ?? null,
    o.lotBorderPatternIndices ?? null,
    o.lotBorderWidths ?? null,
    o.lotSize,
    o.tilePeriod,
    o.baseTileIndex,
  ]);
}

/**
 * LotMask 贴图：加载后按渲染模式应用到地面 fill——默认模式贴服务端合成的
 * 反照率图（引擎口径平色+底图格，缺失时回退量化图），精细模式走引擎语义
 * 合成（composeRefinedGround，Worker 化 + 缓存）。异步完成按 isStale 守卫
 * 丢弃过期代；返回 Promise 供装配层 await（合成成本纳入 scene_rebuild 遥测）。
 */
export async function applyGroundMask(options: {
  THREE: typeof ThreeNamespace;
  ground: ThreeNamespace.Object3D;
  maskPng: string | null;
  /** 默认模式反照率图（服务端 compose_albedo 口径）；null = 回退量化图。 */
  albedoPng?: string | null;
  /** LotSize（米），精细模式的逐轴平铺次数来源。 */
  lotSize?: [number, number] | null;
  /** 地面贴图周期 0x0CCB7FD0（米/格）。 */
  tilePeriod?: [number, number] | null;
  refined: boolean;
  lotColors: [number, number, number, number][];
  /** "Lot Textures" 地表共享纹理像素（真实图集；null = 本地占位 tile）。 */
  surface?: ImageData | null;
  /** 底图格索引（后端三级来源：0x0CCB7FD6 → 推导 → 8）。 */
  baseTileIndex: number;
  /** 全局共享法线图集像素（s15；图案坡度明暗烘焙来源）。 */
  normalAtlas?: ImageData | null;
  /** LotMask 原始通道权重图（阈值选区输入；null = 量化图最近色硬分配）。 */
  rawMask?: ImageData | null;
  /** LotBorderColor1-4 的 sRGB RGB（边框带描边色）。 */
  lotBorderColors?: [number, number, number][];
  /** 边框带图案索引（LotBorderColor.A）。 */
  lotBorderPatternIndices?: number[];
  /** borderWidth1-4（边框带半宽）；全 0 = 无边框。 */
  lotBorderWidths?: number[];
  /** LotOverlayBoxOffset：地面 quad 中心覆盖；null = 引擎回退锚点包围盒中心。 */
  lotOverlayBoxOffset?: [number, number] | null;
  /** 缓存 key 源（源字符串身份；与 ImageData 参数一一对应）。 */
  rawMaskKey?: string | null;
  surfaceKey?: string | null;
  normalAtlasKey?: string | null;
  isStale: () => boolean;
}) {
  const {
    THREE,
    ground,
    maskPng,
    albedoPng,
    lotSize,
    tilePeriod,
    refined,
    lotColors,
    surface,
    baseTileIndex,
    normalAtlas,
    rawMask,
    lotBorderColors,
    lotBorderPatternIndices,
    lotBorderWidths,
    isStale,
  } = options;
  // 默认模式优先用反照率图；精细模式的合成输入仍是量化 mask。
  const flatPng = refined ? maskPng : (albedoPng ?? maskPng);
  if (!flatPng) return;
  const texture = await loadGroundTexture(THREE, flatPng);
  if (isStale()) return;
  texture.colorSpace = THREE.SRGBColorSpace;
  // LotMask 为原始栅格行序（行 0 = 首行）：与模型贴图一致不翻 V，
  // 否则遮罩南北镜像（rendering.md §3.1 遗留项）
  texture.flipY = false;
  const fill = groundFillMesh(ground);
  if (!fill) return;
  if (refined && maskPng) {
    // 精细模式：引擎语义 = 通道 >0.5−bw 阈值 + A>B>G>R 优先级瀑布 →
    // 胜者平色 × 图案坡度明暗（**烘焙进反照率**，lotCalcLighting 的探针
    // 近似口径 = output/lot_hires pattern 同款；normalMap 实时光照在平射
    // 阳光下响应是二阶小量，图案不可见——2026-09-29 用户对拍裁定）；
    // 未覆盖区底图格整格拉伸。
    // compose 成本曾是游离在遥测外的主线程大头——单独纳管成 span。
    const span = renderTelemetry.begin("texture_compose", {
      phase: "ground",
    });
    try {
      const key = groundComposeKey({
        maskPng,
        rawMaskKey: options.rawMaskKey ?? null,
        surfaceKey: options.surfaceKey ?? null,
        normalAtlasKey: options.normalAtlasKey ?? null,
        lotColors,
        lotBorderColors,
        lotBorderPatternIndices,
        lotBorderWidths,
        lotSize,
        tilePeriod,
        baseTileIndex,
      });
      let result = groundComposeCache.get(key);
      const cacheHit = Boolean(result);
      if (!result) {
        const composed = await composeRefinedGround({
          THREE,
          maskImage: texture.image as TexImageSource,
          lotColors,
          lotSize: lotSize ?? null,
          tilePeriod: tilePeriod ?? null,
          surface: surface ?? null,
          baseTileIndex,
          normalAtlas: normalAtlas ?? null,
          rawMask: rawMask ?? null,
          lotBorderColors: lotBorderColors ?? null,
          lotBorderPatternIndices: lotBorderPatternIndices ?? null,
          lotBorderWidths: lotBorderWidths ?? null,
        });
        if (composed) {
          putGroundCache(key, composed);
          result = composed;
        }
      }
      if (isStale() || !result) return;
      const fillMaterial = fill.material as ThreeNamespace.MeshBasicMaterial;
      // 精细地面 = Lambert 受光材质：颜色来自 env 太阳（视口 setSunFromEnv
      // 把 key 光挂到共享 env 的太阳方向/色——与模型注入光照同源，昼夜/亮度
      // 滑杆联动），并接收建筑投影（fill.receiveShadow + viewer 阴影链）。
      // 此前 MeshBasic+env 因子无阴影；更早 MeshPhong+白灯不随昼夜色温变。
      const lit = new THREE.MeshLambertMaterial({ map: result });
      fill.receiveShadow = true;
      fillMaterial.dispose();
      fill.material = lit;
      span.end({ cacheHit });
    } catch {
      span.end({ failed: true });
    }
  } else {
    const material = fill.material as ThreeNamespace.MeshBasicMaterial;
    material.map = texture;
    material.transparent = false;
    material.opacity = 1;
    material.color.set(0xffffff);
    material.needsUpdate = true;
  }
}

/**
 * 【已废弃于渲染，保留作取证工具】mask 空腔质心偏移。统计检验
 * （lot_cavity_stats 400 样本，t=-5.15）否定"建筑锚定空腔质心"假设
 * ——bbox 中心到空腔质心反而比到 lot 中心更远。建筑居中（bbox≈0）。
 */
export function maskCavityOffset(
  maskImage: TexImageSource,
  lotSize: [number, number],
): { x: number; y: number } | null {
  const image = maskImage as { width?: number; height?: number };
  const width = image.width ?? 0;
  const height = image.height ?? 0;
  if (!width || !height) return null;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.drawImage(maskImage as CanvasImageSource, 0, 0);
  const data = context.getImageData(0, 0, width, height).data;
  let sumX = 0;
  let sumY = 0;
  let count = 0;
  const total = width * height;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      if (data[(y * width + x) * 4 + 3] < 16) {
        sumX += x;
        sumY += y;
        count += 1;
      }
    }
  }
  const ratio = count / total;
  if (count < 16 || ratio < 0.01 || ratio > 0.6) return null;
  return {
    x: (sumX / count / width - 0.5) * lotSize[0],
    y: (sumY / count / height - 0.5) * lotSize[1],
  };
}
