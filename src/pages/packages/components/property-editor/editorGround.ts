import { composeRefinedGround } from "./refinedGround";
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
      side: THREE.DoubleSide,
    }),
  );
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
      m[0], m[3], m[6], m[9],
      m[1], m[4], m[7], m[10],
      m[2], m[5], m[8], m[11],
      0, 0, 0, 1,
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

/**
 * LotMask 贴图：加载后按渲染模式应用到地面 fill——精细模式走四色量化
 * 合成（composeRefinedGround），默认模式直接贴 mask。异步完成按 isStale
 * 守卫丢弃过期代。
 */
export function applyGroundMask(options: {
  THREE: typeof ThreeNamespace;
  ground: ThreeNamespace.Object3D;
  maskPng: string;
  refined: boolean;
  lotColors: [number, number, number, number][];
  lotColorsAuthored: boolean[];
  isStale: () => boolean;
}) {
  const { THREE, ground, maskPng, refined, lotColors, lotColorsAuthored, isStale } =
    options;
  new THREE.TextureLoader().load(maskPng, (texture) => {
    if (isStale()) {
      texture.dispose();
      return;
    }
    texture.colorSpace = THREE.SRGBColorSpace;
    // LotMask 为原始栅格行序（行 0 = 首行）：与模型贴图一致不翻 V，
    // 否则遮罩南北镜像（rendering.md §3.1 遗留项）
    texture.flipY = false;
    const fill = groundFillMesh(ground);
    if (!fill) return;
    if (refined) {
      // 精细模式：引擎语义 = 每通道 LotColor.RGB 着色 × LotColor.A 索引的
      // 16 格地面贴图（shader baseTileUVMinMax 4×4 图集；C# lot editor 的
      // GroundTextures 下拉即此 Alpha）。此处按 8 tile/边近似平铺。
      composeRefinedGround(lotColors, lotColorsAuthored, texture.image, THREE)
        .then((map) => {
          if (isStale() || !map) {
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
