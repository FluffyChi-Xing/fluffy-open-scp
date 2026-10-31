/**
 * 地面执行器（three）：把 LotGroundPlan / LotDimensionPlan 变成对象。
 *
 * 与旧 `editorGround.buildLotRect / buildLotDimensions` 的几何/材质逐项对齐；
 * 贴图合成与缓存不在本层（后续由 session resource store 注入）。
 */
import type * as ThreeNamespace from "three";
import type { LotDimensionPlan, LotGroundPlan } from "../../plan/ground";
import { affineToColumnMajor16 } from "../../resolve/matrix";

/** 与 buildLotRect 同参：Z-up 平面、边线 0.05、填充 0.02。 */
export function createLotGround(
  THREE: typeof ThreeNamespace,
  plan: LotGroundPlan,
): ThreeNamespace.Group {
  const group = new THREE.Group();
  const border = new THREE.LineLoop(
    new THREE.BufferGeometry().setFromPoints(
      plan.corners.map((point) => new THREE.Vector3(point[0], point[1], point[2])),
    ),
    new THREE.LineBasicMaterial({ color: 0x9aa4b8 }),
  );
  const fill = new THREE.Mesh(
    new THREE.PlaneGeometry(plan.width, plan.depth),
    new THREE.MeshBasicMaterial({
      color: 0x6b7689,
      transparent: true,
      opacity: 0.08,
      side: THREE.FrontSide,
      // 引擎 preview PS 有 clip(alpha - 1/255)：近零覆盖像素镂空
      alphaTest: 1 / 255,
    }),
  );
  fill.position.z = plan.fillZ;
  group.add(border, fill);
  if (plan.placementInverse) {
    group.matrixAutoUpdate = false;
    group.matrix.fromArray(affineToColumnMajor16(plan.placementInverse));
  }
  return group;
}

/** 与 buildLotDimensions 同参：LineSegments + 端部刻线。 */
export function createLotDimensions(
  THREE: typeof ThreeNamespace,
  plan: LotDimensionPlan,
): { object: ThreeNamespace.LineSegments; anchors: LotDimensionPlan["anchors"] } {
  const geometry = new THREE.BufferGeometry();
  const flat: number[] = [];
  for (const point of plan.points) flat.push(point[0], point[1], point[2]);
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(flat, 3));
  const object = new THREE.LineSegments(
    geometry,
    new THREE.LineBasicMaterial({ color: 0x0878fe }),
  );
  object.name = "__lotDimensions";
  return { object, anchors: plan.anchors };
}
