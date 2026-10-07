import * as THREE from "three";
import { describe, expect, it } from "vitest";
import {
  collectNodeTransfers,
  extractObjectTree,
  nodesToObjects,
} from "./gltfParseDto";

/** 构造测试树：root Group → [mesh0(BoxGeometry), group → mesh1(带 COLOR_0)]。 */
function buildTree(): THREE.Object3D {
  const root = new THREE.Group();
  root.name = "root";
  root.position.set(1, 2, 3);

  const addUv1 = (geometry: THREE.BufferGeometry) => {
    // TEXCOORD_1 = (materialIndex/255, interiorSeed/255, 0, 0)——tint 着色器
    // 按顶点选 regionXform 行的关键属性（漏搬 = 贴图整面采错区域）
    const count = geometry.getAttribute("position").count;
    const uv1 = new Float32Array(count * 4);
    for (let i = 0; i < count; i += 1) uv1[i * 4] = (i % 4) / 255;
    geometry.setAttribute("uv1", new THREE.BufferAttribute(uv1, 4));
  };

  const box = new THREE.BoxGeometry(2, 2, 2);
  addUv1(box);
  const mesh0 = new THREE.Mesh(box);
  mesh0.name = "mesh0";
  mesh0.position.set(4, 5, 6);
  mesh0.rotation.set(0.1, 0.2, 0.3);

  const nested = new THREE.Group();
  nested.name = "nested";
  nested.scale.set(2, 2, 2);

  const plane = new THREE.PlaneGeometry(1, 1);
  const color = new Float32Array(plane.getAttribute("position").count * 4);
  for (let i = 0; i < color.length; i += 1) color[i + 3] = 255;
  plane.setAttribute("color", new THREE.BufferAttribute(color, 4));
  addUv1(plane);
  const mesh1 = new THREE.Mesh(plane);
  mesh1.name = "mesh1";

  nested.add(mesh1);
  root.add(mesh0, nested);
  return root;
}

describe("gltfParseDto extract/rebuild roundtrip", () => {
  it("树结构/命名/父子关系保真", () => {
    const root = buildTree();
    const nodes = extractObjectTree(root, THREE);
    const rebuilt = nodesToObjects(nodes, THREE);
    expect(rebuilt).toHaveLength(1);
    const newRoot = rebuilt[0];
    expect(newRoot.name).toBe("root");
    expect(newRoot.children).toHaveLength(2);
    expect(newRoot.children[0].name).toBe("mesh0");
    expect(newRoot.children[1].name).toBe("nested");
    expect(newRoot.children[1].children[0].name).toBe("mesh1");
  });

  it("变换逐值保真（position/quaternion/scale 经矩阵 roundtrip）", () => {
    const root = buildTree();
    const nodes = extractObjectTree(root, THREE);
    const rebuilt = nodesToObjects(nodes, THREE);
    const newRoot = rebuilt[0];
    expect(newRoot.position.toArray()).toEqual(root.position.toArray());
    const mesh0 = newRoot.children[0] as THREE.Mesh;
    const oldMesh0 = (root.children[0] as THREE.Mesh);
    expect(mesh0.position.toArray()).toEqual(oldMesh0.position.toArray());
    // transform 以 f32 存储：四元数 roundtrip 有 1e-8 级精度损失（预期）
    mesh0.quaternion.toArray().forEach((value, index) => {
      expect(value).toBeCloseTo(oldMesh0.quaternion.toArray()[index], 6);
    });
    const nested = newRoot.children[1];
    expect(nested.scale.toArray()).toEqual(
      (root.children[1] as THREE.Object3D).scale.toArray(),
    );
  });

  it("几何属性 buffer 保真（position/uv/index/COLOR_0 itemSize=4）", () => {
    const root = buildTree();
    const nodes = extractObjectTree(root, THREE);
    const rebuilt = nodesToObjects(nodes, THREE);
    const mesh0 = rebuilt[0].children[0] as THREE.Mesh;
    const oldMesh0 = root.children[0] as THREE.Mesh;
    const newPos = mesh0.geometry.getAttribute("position");
    const oldPos = oldMesh0.geometry.getAttribute("position");
    expect(newPos.array).toEqual(oldPos.array);
    expect(mesh0.geometry.getAttribute("uv").array).toEqual(
      oldMesh0.geometry.getAttribute("uv").array,
    );
    expect(mesh0.geometry.getAttribute("uv1").array).toEqual(
      oldMesh0.geometry.getAttribute("uv1").array,
    );
    expect(mesh0.geometry.getAttribute("uv1").itemSize).toBe(4);
    expect(mesh0.geometry.index?.array).toEqual(
      oldMesh0.geometry.index?.array,
    );
    const mesh1 = (rebuilt[0].children[1] as THREE.Group).children[0] as THREE.Mesh;
    const oldMesh1 = (root.children[1] as THREE.Group).children[0] as THREE.Mesh;
    expect(mesh1.geometry.getAttribute("color").itemSize).toBe(4);
    expect(mesh1.geometry.getAttribute("color").array).toEqual(
      oldMesh1.geometry.getAttribute("color").array,
    );
  });

  it("collectNodeTransfers 覆盖全部属性/索引/变换 buffer", () => {
    const root = buildTree();
    const nodes = extractObjectTree(root, THREE);
    const transfers = collectNodeTransfers(nodes);
    const buffers = new Set(transfers);
    for (const node of nodes) {
      expect(buffers.has(node.transform.buffer)).toBe(true);
      for (const attribute of Object.values(node.attributes)) {
        expect(buffers.has(attribute.array.buffer)).toBe(true);
      }
      if (node.index) {
        expect(buffers.has(node.index.buffer)).toBe(true);
      }
    }
  });

  it("重建产物可正常绘制（无缺失 position）", () => {
    const root = buildTree();
    const nodes = extractObjectTree(root, THREE);
    const rebuilt = nodesToObjects(nodes, THREE);
    rebuilt[0].traverse((child) => {
      const mesh = child as THREE.Mesh;
      if (mesh.isMesh) {
        expect(mesh.geometry.getAttribute("position")).toBeDefined();
        expect(mesh.geometry.drawRange.count).not.toBe(0);
      }
    });
  });
});
