import type * as ThreeNamespace from "three";
import type { LotModelPayload } from "@/api/tauri";

/** 与 Rust 侧 `LOT_MODEL_PAYLOAD_MAGIC` 一致："LOTM"（小端字节序）。 */
const PAYLOAD_MAGIC = 0x4d54_4f4c;

/**
 * 解析 `read_lot_model_meshes` 返回的原始字节容器（零拷贝切片，
 * 取代旧 base64+JSON 通道；见 `LotModelPayload` 注释里的容器布局）。
 */
export function parseLotModelContainer(buffer: ArrayBuffer): LotModelPayload {
  const view = new DataView(buffer);
  if (buffer.byteLength < 21) {
    throw new Error("lot model payload truncated");
  }
  let offset = 0;
  const readU32 = () => {
    const value = view.getUint32(offset, true);
    offset += 4;
    return value;
  };
  if (readU32() !== PAYLOAD_MAGIC) {
    throw new Error("lot model payload magic mismatch");
  }
  const version = readU32();
  if (version !== 1) {
    throw new Error(`unsupported lot model payload version ${version}`);
  }
  const meshCount = readU32();
  const glbs: ArrayBuffer[] = [];
  for (let index = 0; index < meshCount; index += 1) {
    const length = readU32();
    if (offset + length > buffer.byteLength) {
      throw new Error("lot model payload mesh out of bounds");
    }
    glbs.push(buffer.slice(offset, offset + length));
    offset += length;
  }
  const readPng = (): Uint8Array<ArrayBuffer> | null => {
    const length = readU32();
    if (length === 0) return null;
    if (offset + length > buffer.byteLength) {
      throw new Error("lot model payload texture out of bounds");
    }
    const bytes = buffer.slice(offset, offset + length);
    offset += length;
    return new Uint8Array(bytes);
  };
  const baseColorPng = readPng();
  const normalPng = readPng();
  const hasUv = view.getUint8(offset) !== 0;
  return { glbs, baseColorPng, normalPng, hasUv };
}

/** PNG 字节 → blob URL（TextureLoader 可直接加载，免去 data:URL base64 再解码）。 */
export function pngBlobUrl(bytes: Uint8Array<ArrayBuffer>): string {
  return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
}

/**
 * 解析地块模型的全部 GLB 为 Object3D。
 * gltf.rs 根节点自带 Z-up→Y-up 的 -90°X 旋转，而视口 world 组已做同款
 * 旋转（模型须与 Unit gizmo 共享 Z-up 世界），这里剥掉根旋转避免双重旋转。
 */
export async function parseLotModelObjects(
  glbs: ArrayBuffer[],
): Promise<ThreeNamespace.Object3D[]> {
  const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
  const loader = new GLTFLoader();
  const objects: ThreeNamespace.Object3D[] = [];
  for (const glb of glbs) {
    const gltf = await loader.parseAsync(glb, "");
    for (const child of gltf.scene.children) {
      child.rotation.set(0, 0, 0);
    }
    objects.push(gltf.scene);
  }
  return objects;
}
