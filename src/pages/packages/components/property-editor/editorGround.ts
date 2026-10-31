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
  // 后端已将原始遮罩翻行：PlaneGeometry 的 V 沿 +Y，纹理 flipY=false。
  // 引擎的原始遮罩 V 沿 -Y；此处无需再次翻转。
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

export {
  applyGroundTextures as applyGroundMask,
  releaseGroundTextureCache as releaseGroundComposeCache,
} from "@/render/exec/three/groundTextures";

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
