import type * as ThreeNamespace from "three";

export function decodeBase64(base64: string): Uint8Array<ArrayBuffer> {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/** 解析 RW4 导出的 OBJ 文本为白模 Object3D（共享 MeshStandardMaterial，补算缺失法线）。 */
export async function parseObjModel(
  objBase64: string,
): Promise<ThreeNamespace.Object3D> {
  const THREE = await import("three");
  const { OBJLoader } = await import("three/examples/jsm/loaders/OBJLoader.js");
  const text = new TextDecoder().decode(decodeBase64(objBase64));
  const object = new OBJLoader().parse(text);
  const material = new THREE.MeshStandardMaterial({
    color: 0xb8c2cc,
    roughness: 0.55,
    metalness: 0.12,
    side: THREE.DoubleSide,
  });
  object.traverse((child) => {
    const mesh = child as ThreeNamespace.Mesh;
    if (mesh.isMesh) {
      mesh.material = material;
      if (!mesh.geometry.attributes.normal) mesh.geometry.computeVertexNormals();
    }
  });
  return object;
}
