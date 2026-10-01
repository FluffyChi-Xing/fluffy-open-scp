import type * as ThreeNamespace from "three";
import type { DecalUnit } from "@/api/tauri";

type Three = typeof ThreeNamespace;

/**
 * 贴花在 lot 局部空间的投影坐标系。
 *
 * 12 float 变换是 WPF Matrix3D 行主序（行向量约定）：前 3 行是基向量、末行是平移。
 * 脱壳 exe 的 `decalMaterialInfoWithObjectData` 把贴花投影轴取为基的第三行，
 * 而 2026-09-14 用真实 lot（casino `0x457EA9DB`，模型 0x01532F56）实测：
 * 足迹内 3×3 射线**全部命中 +局部Z**且距离高度一致（离散度 0.00–2.05 m），
 * 即投影方向是 **+Z**，目标是一块平面。
 *
 * ## 引擎对齐（2026-10-01 三源互证重分析）
 *
 * - **Ghidra `SC_cVolumeDecalManager.c` FUN_006fdce0**：decal 记录矩阵由
 *   unit 字段**直接构造**（基 = 旋转 × scale × 0.5 × 10，平移 = 位置 +
 *   旋转偏移 × scale），全程**无射线检测、无距离判定**；唯一的条件是
 *   可见类别 0xCAAD8CA > 0（exe 常量 DAT_00cf1e4c = 0.0f）。
 * - **shader 容器完整源码**：`decalClip`(PS) = `clip(-textureFloatPosition.z)`
 *   半空间裁剪 + `uv = texpos.xy×-0.5+0.5`（×texXform atlas 格）。
 *   早先记在 decalProject 名下的 `clip(1-abs(texcoord))` 实属
 *   `regionDecalProject`（区域 decal）与 `decalSDF`（调试可视化）。
 * - 因此**锚定恒为变换原点**——"贴在墙上"是数据作者把原点放在墙上的结果，
 *   招牌与墙面的小间距在游戏内同样直接渲染（引擎无吸附判定）。
 */
export interface DecalFrame {
  /** 贴花原点（lot 局部）。 */
  origin: ThreeNamespace.Vector3;
  /** 单位基向量（u / v / 投影轴）。 */
  axisX: ThreeNamespace.Vector3;
  axisY: ThreeNamespace.Vector3;
  axisZ: ThreeNamespace.Vector3;
  /** lot 局部变换矩阵（位置 + 朝向，无缩放）。 */
  matrix: ThreeNamespace.Matrix4;
  /** 投影盒 XY 全尺寸：`height = 2×scale`（scale = 半高），`width = height×aspect`。 */
  sizeX: number;
  sizeY: number;
}

/**
 * 盒的 Z 半厚下限（米）。只用于破洞体积盒/浮空 quad 回退的材质参数；
 * 投影盒厚度由引擎体积盒语义给出（见 BOX_HALF_NOTE），与 depth 无关。
 */
const HALF_THICKNESS_MIN = 0.05;

/** 投影盒半厚（米）= depth 原值（origin→墙面距离，§41.2 语义）。 */
export function decalHalfThickness(depth: number | null): number {
  return Math.max(depth ?? 0, HALF_THICKNESS_MIN);
}

/**
 * 引擎体积盒（2026-10-01 扫查实证，docs/re/decal-engine-alignment.md）：
 *
 * C 侧 FUN_006fdce0 的盒基 = 旋转 × scale × 0.5 × 10 → **Z 全深 = 5×scale**
 * （三轴同系数；横向系数是否同为 ×5 静态不可判——unit 旋转行可能携带逐轴
 * 长度，见文档 §五）。120 lot 扫查：wall 族原点 90% 在建筑 bbox 内、离最近
 * 面中位 2.5~4.5m——数据作者不做厘米级贴墙，"吸附"全靠这个大盒在渲染期
 * 罩住立面（延迟模式 = 最近深度）。薄盒按 depth 摆放是此前"漂浮/漏投"的
 * 最后根源：原点在墙内 2.6m、薄盒 1m 深 → 够不着任何面。
 * 横向维持可见窗 `2×scale×aspect`（OMEGACO 游戏实测校准）——若引擎横向
 * 真为 5×scale+texXform 窗口，需运行时 VB 对拍后再改。
 */
export const DECAL_BOX_DEPTH_FACTOR = 2.5;

export function decalFrame(
  THREE: Three,
  unit: DecalUnit,
  aspectRatio: number,
): DecalFrame | null {
  const m = unit.transform?.matrix;
  if (!m || m.length !== 12) return null;
  const scale = unit.scale;
  if (scale === null || !Number.isFinite(scale) || scale <= 0) return null;
  const aspect = aspectRatio > 0 && Number.isFinite(aspectRatio) ? aspectRatio : 1;

  const axisX = new THREE.Vector3(m[0], m[1], m[2]);
  const axisY = new THREE.Vector3(m[3], m[4], m[5]);
  const axisZ = new THREE.Vector3(m[6], m[7], m[8]);
  const origin = new THREE.Vector3(m[9], m[10], m[11]);
  // 正交基（实测 |g| = 1）；退化时归一化保护
  axisX.normalize();
  axisY.normalize();
  axisZ.normalize();

  // 行主序 → three 列向量约定（同 unitGizmos.unitMatrix）
  const matrix = new THREE.Matrix4().set(
    m[0], m[3], m[6], m[9],
    m[1], m[4], m[7], m[10],
    m[2], m[5], m[8], m[11],
    0, 0, 0, 1,
  );

  // 可见尺寸语义（2026-09-27 OMEGACO 对照定谳）：scale 是**半高**——
  // 广告可见窗高 = 2×scale、宽 = 高×aspect。尺寸与广告牌/招牌模型**无关**
  // ——引擎体积盒完全由变换数据给出（FUN_006fdce0 不读任何模型尺寸）。
  const sizeY = Math.max(scale * 2, 0.05);
  return { origin, axisX, axisY, axisZ, matrix, sizeX: Math.max(sizeY * aspect, 0.05), sizeY };
}

/**
 * 生成投影盒（lot 局部）：中心恒为变换原点（引擎同构，无射线/搜索），
 * 横向 = 可见窗 `2×scale×aspect`，Z 全深 = 引擎体积盒 `5×scale`
 * （基系数 0.5×10，见 DECAL_BOX_DEPTH_FACTOR）。
 * 不剔背面是照抄引擎 `decalClip` 的 `clip(-z)`（无法线判定）；背面
 */
export function decalProjector(
  THREE: Three,
  frame: DecalFrame,
): {
  position: ThreeNamespace.Vector3;
  orientation: ThreeNamespace.Euler;
  size: ThreeNamespace.Vector3;
} {
  const orientation = new THREE.Euler().setFromRotationMatrix(frame.matrix);
  const size = new THREE.Vector3(
    frame.sizeX,
    frame.sizeY,
    frame.sizeY * DECAL_BOX_DEPTH_FACTOR,
  );
  return { position: frame.origin.clone(), orientation, size };
}

/**
 * 体积内最近面吸附（引擎延迟投影语义的 CPU 等价，2026-10-01 定稿）：
 * 自变换原点沿 ±体积轴投射，距离上限 = 引擎体积盒半深（2.5×scale，
 * FUN_006fdce0 基=R×scale×0.5×10 的 Z 半深）——即"体积盒内最近可见面"
 * 的射线代理。命中 → quad 贴该面（招牌观感，等价引擎延迟投影落墙）；
 * 未命中 → quad 停在原点（全息浮空 = 引擎前向路径语义）。
 *
 * 这不是发明阈值：距离上限就是引擎体积盒自身的深度，超出即引擎同样
 * 不会投影的范围。
 */
export function snapQuadToSurface(
  THREE: Three,
  frame: DecalFrame,
  proxies: ThreeNamespace.Mesh[],
): { position: ThreeNamespace.Vector3; hit: boolean } {
  const raycaster = new THREE.Raycaster();
  const halfDepth = frame.sizeY * 2.5 * 0.5; // 2.5×scale（体积盒半深）
  raycaster.far = halfDepth;
  // 优先 -axisZ：引擎 decal 朝向约定（decalMaterialInfoWithObjectData 的
  // VS 取 -z 为面向）——招牌面片面向街侧，吸附应优先落在面向方向上；
  // 无命中再试 +axisZ（原点在墙内、立面在面向反侧的摆放形态）。
  for (const sign of [-1, 1] as const) {
    const dir = frame.axisZ.clone().multiplyScalar(sign);
    raycaster.set(frame.origin, dir);
    const hit = raycaster.intersectObjects(proxies, false)[0];
    if (hit) {
      // 贴面时向原点侧回撤 3cm 防 z-fight（材质另有 polygonOffset）
      const position = frame.origin
        .clone()
        .addScaledVector(dir, Math.max(hit.distance - 0.03, 0.01));
      return { position, hit: true };
    }
  }
  return { position: frame.origin.clone(), hit: false };
}
