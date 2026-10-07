import type * as ThreeNamespace from "three";

/**
 * GLB 解析 Worker 的共享 DTO 与主线程重建（three-gltf.ts 的 Worker 化）。
 *
 * 背景：建筑/prop 的 GLB 由自家 Rust 导出器产出（package_service 载荷：
 * `EmbeddedTextures::default()` = **零内嵌纹理**，`skeleton=None anims=&[]`
 * = 纯静态网格）——Worker 侧 GLTFLoader 无任何 DOM/纹理依赖，解析结果
 * 提取为「扁平节点表 + transferable 属性 buffer」转移回主线程重建
 * Object3D。主线程只付 memcpy 级重建，不再承担数百 ms 的 GLB 同步解析。
 *
 * 根旋转剥离约定（与主线程旧路径逐值等价）：worker 在提取前对
 * `gltf.scene.children` 执行 `rotation.set(0,0,0)`——gltf.rs 根节点的
 * Z-up→Y-up 旋转由视口 world 组承担，双重旋转须剥掉。
 */

/** 扁平节点（前序遍历；parent = 节点表下标，-1 = 根级）。 */
export interface GltfNodeDto {
  parent: number;
  name: string;
  isMesh: boolean;
  /** 列主序 Matrix4.elements 16 项。 */
  transform: Float32Array;
  position?: Float32Array;
  normal?: Float32Array;
  uv?: Float32Array;
  /** COLOR_0 顶点色（3 或 4 分量，见 colorItemSize）。 */
  color?: Float32Array;
  colorItemSize?: number;
  index?: Uint32Array | Uint16Array;
}

/** Worker → 主线程应答。 */
export interface GltfParseResponse {
  id: number;
  nodes?: GltfNodeDto[];
  error?: string;
}

/** Worker 请求。 */
export interface GltfParseRequest {
  id: number;
  /** GLB 字节（调用方保证为副本——转移后本侧 detach）。 */
  glbs: ArrayBuffer[];
}

/** 收集全部可转移 buffer（属性/索引/变换）。 */
export function collectNodeTransfers(nodes: GltfNodeDto[]): ArrayBufferLike[] {
  const transfers = new Set<ArrayBufferLike>();
  for (const node of nodes) {
    transfers.add(node.transform.buffer);
    for (const array of [
      node.position,
      node.normal,
      node.uv,
      node.color,
      node.index,
    ]) {
      if (array) transfers.add(array.buffer);
    }
  }
  return [...transfers];
}

/**
 * Object3D 树 → 扁平节点 DTO（worker 侧提取；纯逻辑可测）。
 * object 自身作为根节点（parent -1）纳入——与主线程旧路径"每 GLB 返回
 * 一个 gltf.scene 包装"逐值对齐。仅认 Mesh/Group，其余类型（灯光/相机等，
 * 导出器不产出）跳过。
 */
export function extractObjectTree(
  object: ThreeNamespace.Object3D,
  THREE: typeof ThreeNamespace,
): GltfNodeDto[] {
  const nodes: GltfNodeDto[] = [];
  const walk = (node: ThreeNamespace.Object3D, parent: number): void => {
    const mesh = node as ThreeNamespace.Mesh;
    const isMesh = mesh.isMesh === true && mesh.geometry !== undefined;
    const matrix = new THREE.Matrix4().compose(
      node.position,
      node.quaternion,
      node.scale,
    );
    const dto: GltfNodeDto = {
      parent,
      name: node.name,
      isMesh,
      transform: new Float32Array(matrix.elements),
    };
    const index = nodes.push(dto) - 1;
    if (isMesh) {
      const geometry = mesh.geometry;
      const position = geometry.getAttribute("position");
      dto.position = position
        ? (position.array as Float32Array)
        : new Float32Array(0);
      const normal = geometry.getAttribute("normal");
      dto.normal = normal ? (normal.array as Float32Array) : undefined;
      const uv = geometry.getAttribute("uv");
      dto.uv = uv ? (uv.array as Float32Array) : undefined;
      const color = geometry.getAttribute("color");
      if (color) {
        dto.color = color.array as Float32Array;
        dto.colorItemSize = color.itemSize;
      }
      dto.index = geometry.index
        ? (geometry.index.array as Uint32Array | Uint16Array)
        : undefined;
    }
    for (const child of node.children) walk(child, index);
  };
  walk(object, -1);
  return nodes;
}

let placeholderMaterial: ThreeNamespace.Material | null = null;
function placeholder(
  THREE: typeof ThreeNamespace,
): ThreeNamespace.Material {
  // 调用方（viewport/propModels/treeRenderer）装配时都会逐 mesh 覆盖材质，
  // 占位只保证重建产物可被 clone/渲染；全树共享一个实例省内存。
  placeholderMaterial ??= new THREE.MeshStandardMaterial({
    color: 0xffffff,
    roughness: 0.8,
    metalness: 0,
    side: THREE.DoubleSide,
  });
  return placeholderMaterial;
}

/** 扁平节点表 → Object3D 根数组（主线程重建；buffer 零拷贝入 BufferAttribute）。 */
export function nodesToObjects(
  nodes: GltfNodeDto[],
  THREE: typeof ThreeNamespace,
): ThreeNamespace.Object3D[] {
  const created: ThreeNamespace.Object3D[] = [];
  const roots: ThreeNamespace.Object3D[] = [];
  for (const node of nodes) {
    let object: ThreeNamespace.Object3D;
    if (node.isMesh) {
      const geometry = new THREE.BufferGeometry();
      if (node.position) {
        geometry.setAttribute(
          "position",
          new THREE.BufferAttribute(node.position, 3),
        );
      }
      if (node.normal) {
        geometry.setAttribute(
          "normal",
          new THREE.BufferAttribute(node.normal, 3),
        );
      }
      if (node.uv) {
        geometry.setAttribute("uv", new THREE.BufferAttribute(node.uv, 2));
      }
      if (node.color) {
        geometry.setAttribute(
          "color",
          new THREE.BufferAttribute(
            node.color,
            node.colorItemSize ?? 3,
          ),
        );
      }
      if (node.index) {
        geometry.setIndex(new THREE.BufferAttribute(node.index, 1));
      }
      const mesh = new THREE.Mesh(geometry, placeholder(THREE));
      mesh.castShadow = false;
      object = mesh;
    } else {
      object = new THREE.Group();
    }
    object.name = node.name;
    new THREE.Matrix4()
      .fromArray(node.transform)
      .decompose(object.position, object.quaternion, object.scale);
    created.push(object);
    if (node.parent >= 0 && created[node.parent]) {
      created[node.parent].add(object);
    } else {
      roots.push(object);
    }
  }
  return roots;
}
