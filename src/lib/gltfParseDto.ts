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
 *
 * **属性必须全量搬运（2026-10-07 勘误）**：导出器写的不止 POSITION/
 * NORMAL/TEXCOORD_0——TEXCOORD_1 = 逐顶点材质索引+内景种子（前端 tint
 * 着色器按 uv1.x 选 regionXform 行）、TEXCOORD_2/3 = facade 世界投影
 * 双 UV 域（Base/Top 层）、COLOR_0 = 顶点色（可能归一化 UByte）。GLTFLoader
 * 命名 TEXCOORD_0→uv、TEXCOORD_n→uvN——属性表按名全量转移，itemSize/
 * normalized 一并保留，漏一个 = 贴图整面采错区域（真机：建筑全灰）。
 */

/** 单个顶点属性：按 accessor 组件类型的数组 + 布局元数据。 */
export interface GltfAttributeDto {
  array: Float32Array | Uint16Array | Uint8Array;
  itemSize: number;
  /** COLOR_0 归一化 UByte 时为 true（漏掉 = 顶点色爆表）。 */
  normalized?: boolean;
}

/** 扁平节点（前序遍历；parent = 节点表下标，-1 = 根级）。 */
export interface GltfNodeDto {
  parent: number;
  name: string;
  isMesh: boolean;
  /** 列主序 Matrix4.elements 16 项。 */
  transform: Float32Array;
  /** 顶点属性按 GLTFLoader 命名全量搬运（position/normal/uv/uv1..3/color）。 */
  attributes: Record<string, GltfAttributeDto>;
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
    for (const attribute of Object.values(node.attributes)) {
      transfers.add(attribute.array.buffer);
    }
    if (node.index) transfers.add(node.index.buffer);
  }
  return [...transfers];
}

/** BufferAttribute/InterleavedBufferAttribute → 独立 DTO（必要时去交错）。 */
function attributeToDto(
  attribute: ThreeNamespace.BufferAttribute,
): GltfAttributeDto {
  // 导出器写非交错 accessor；InterleavedBufferAttribute（GLB 共享
  // bufferView 时 GLTFLoader 产出）无 .array，按 getX 逐顶点去交错。
  const interleaved = attribute as unknown as {
    isInterleavedBufferAttribute?: boolean;
    data?: { array: Float32Array | Uint16Array | Uint8Array };
    stride?: number;
    offset?: number;
  };
  if (interleaved.isInterleavedBufferAttribute && interleaved.data) {
    const count = attribute.count;
    const itemSize = attribute.itemSize;
    const ArrayCtor = interleaved.data.array.constructor as
      | Float32ArrayConstructor
      | Uint16ArrayConstructor
      | Uint8ArrayConstructor;
    const out = new ArrayCtor(count * itemSize);
    for (let i = 0; i < count; i += 1) {
      for (let c = 0; c < itemSize; c += 1) {
        out[i * itemSize + c] = attribute.getComponent(i, c);
      }
    }
    return {
      array: out,
      itemSize,
      normalized: attribute.normalized || undefined,
    };
  }
  return {
    array: attribute.array as Float32Array,
    itemSize: attribute.itemSize,
    normalized: attribute.normalized || undefined,
  };
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
      attributes: {},
    };
    const index = nodes.push(dto) - 1;
    if (isMesh) {
      const geometry = mesh.geometry;
      for (const [name, attribute] of Object.entries(geometry.attributes)) {
        dto.attributes[name] = attributeToDto(
          attribute as ThreeNamespace.BufferAttribute,
        );
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
      for (const [name, attribute] of Object.entries(node.attributes)) {
        geometry.setAttribute(
          name,
          new THREE.BufferAttribute(
            attribute.array,
            attribute.itemSize,
            attribute.normalized,
          ),
        );
      }
      if (node.index) {
        geometry.setIndex(new THREE.BufferAttribute(node.index, 1));
      }
      const mesh = new THREE.Mesh(geometry, placeholder(THREE));
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
