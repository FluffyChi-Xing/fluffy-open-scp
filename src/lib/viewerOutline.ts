import type * as ThreeNamespace from "three";

/** 主题色描边（品牌色 #0878FE，与 UI --brand 一致）。 */
export const OUTLINE_COLOR = 0x0878fe;

/**
 * 悬停/选中轮廓壳（反转壳体法）：把目标网格克隆为「法线外扩 + BackSide +
 * 纯色」的壳，挂在目标对象下继承变换——轮廓贴着模型剪影走，不随视角变形
 * （2026-10-06 取代屏幕空间 AABB 方框：方框大小不定且精细灯光 bbox 爆炸）。
 * hover=虚线、selected=实线。
 *
 * 纯构建函数（THREE / 取景半径显式传入），便于单测与视口解耦。
 */
export function buildOutlineHull(
  THREE: typeof ThreeNamespace,
  target: ThreeNamespace.Object3D,
  kind: "hover" | "selected",
  frameRadius: number,
): ThreeNamespace.Object3D {
  const dashed = kind === "hover";
  // 壳厚随取景半径缩放（近似恒定屏幕厚度）
  const width = Math.max(frameRadius, 1) * (dashed ? 0.005 : 0.01);
  // 壳体逐 mesh 构建以继承各自的漫反射贴图——alpha 镂空网格（树叶卡）
  // 的壳体须同步 alphaTest 抠透，否则描边色从镂空缝隙整面透出
  //（真机：树冠蓝色棋盘格，2026-10-06）。
  const shellMaterial = (mesh: ThreeNamespace.Mesh) => {
    const srcMaterial = Array.isArray(mesh.material)
      ? mesh.material[0]
      : mesh.material;
    const map =
      (srcMaterial as ThreeNamespace.MeshStandardMaterial | undefined)?.map ??
      null;
    return new THREE.ShaderMaterial({
      uniforms: {
        uColor: { value: new THREE.Color(OUTLINE_COLOR) },
        uWidth: { value: width },
        uDash: { value: dashed ? 1 : 0 },
        uMap: { value: map },
        uHasMap: { value: map ? 1 : 0 },
      },
      vertexShader: `
        varying vec2 vUv;
        uniform float uWidth;
        void main() {
          vUv = uv;
          vec4 mv = modelViewMatrix * vec4(position, 1.0);
          vec3 n = normalize(normalMatrix * normal);
          mv.xyz += n * uWidth;
          gl_Position = projectionMatrix * mv;
        }`,
      fragmentShader: `
        varying vec2 vUv;
        uniform vec3 uColor;
        uniform float uDash;
        uniform sampler2D uMap;
        uniform float uHasMap;
        void main() {
          if (uHasMap > 0.5 && texture2D(uMap, vUv).a < 0.5) discard;
          if (uDash > 0.5 && mod(floor(gl_FragCoord.x / 9.0) + floor(gl_FragCoord.y / 9.0), 2.0) < 1.0) discard;
          gl_FragColor = vec4(uColor, 1.0);
        }`,
      side: THREE.BackSide,
    });
  };
  const shellFor = (mesh: ThreeNamespace.Mesh, inheritTransform: boolean) => {
    const shell = new THREE.Mesh(mesh.geometry, shellMaterial(mesh));
    if (inheritTransform) {
      shell.position.copy(mesh.position);
      shell.quaternion.copy(mesh.quaternion);
      shell.scale.copy(mesh.scale);
    }
    return shell;
  };
  const hull = new THREE.Group();
  hull.name = "__outlineHull";
  // 目标本身是 Mesh（锥体/占位人形等 buildUnitObject 直出）：壳挂恒等
  // 变换——hull 已是 target 子级，target 自带变换；再复制本地变换会双重
  // 叠加（真机：轮廓偏移到两倍位置）。
  if ((target as ThreeNamespace.Mesh).isMesh) {
    hull.add(shellFor(target as ThreeNamespace.Mesh, false));
    return hull;
  }
  const walk = (src: ThreeNamespace.Object3D, dst: ThreeNamespace.Object3D) => {
    for (const child of [...src.children]) {
      const mesh = child as ThreeNamespace.Mesh;
      // Editor chrome (light volume beams, invisible pick proxies) must not
      // contribute an outline shell: a selected spot light would otherwise be
      // wrapped in a solid cone the size of its beam.
      if (mesh.isMesh && !mesh.userData.pickOnly) {
        dst.add(shellFor(mesh, true));
      } else if (
        (child as ThreeNamespace.Object3D & { isGroup?: boolean }).isGroup
      ) {
        const group = new THREE.Group();
        group.position.copy(child.position);
        group.quaternion.copy(child.quaternion);
        group.scale.copy(child.scale);
        dst.add(group);
        walk(child, group);
      }
      // 光源/线条等其他类型跳过（路径线无轮廓语义）
    }
  };
  walk(target, hull);
  return hull;
}
