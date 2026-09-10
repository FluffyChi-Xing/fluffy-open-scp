import * as THREE from "three";
import { tauriApi } from "@/api";
import { command } from "@/api/tauri";
import { parseLotModelObjects, pngBlobUrl } from "@/lib/three-gltf";
import type { LotModelPayload } from "@/api/tauri";
import { useToast } from "@/composables/useToast";

/**
 * 编辑器 → 外部建模软件（Blender 等）的模型导出。
 *
 * 数据链：read_lot_model_meshes（RW4 模型 TGI）→ LOTM 载荷内建的标准
 * GLB 网格（COLOR_0 烘焙顶点色）→ three.js 场景 → GLTFExporter(GLB) →
 * write_export_file 落盘（路径来自保存对话框）。
 *
 * 模式：
 * - white：纯网格 + 平光白材质（编辑器默认白模对齐）；
 * - textured：按 0x2001A 材质绑定附 baseColor/normal/roughness/AO 贴图。
 *   uvKind=2（facade tint 着色器）网张无法用标准 PBR 表达 tint 查表链，
 *   保留 COLOR_0 顶点色近似（与编辑器白模同观感）——tint 链的逐像素
 *   结果只存在于自定义 shader 中，标准格式不可承载。
 */

export type ModelExportMode = "white" | "textured";

async function loadTexture(bytes: Uint8Array<ArrayBuffer>): Promise<THREE.Texture | null> {
  const url = pngBlobUrl(bytes);
  try {
    return await new Promise((resolve, reject) => {
      new THREE.TextureLoader().load(url, resolve, undefined, reject);
    });
  } catch {
    return null;
  }
}

/**
 * 导出前几何清洗：LOTM GLB 携带 VEC4 的 TEXCOORD_1/2/3（材质索引/种子/
 * facade 世界投影 UV），glTF 规范要求 UV 访问器为 VEC2——Blender 5.1
 * 导入器合并 UV 数组时维度不匹配直接崩溃（用户实测）。只保留
 * position/normal/uv/color；uv 若为 4 分量截取 xy。
 */
function sanitizeGeometry(geometry: THREE.BufferGeometry): void {
  const keep = new Set(["position", "normal", "uv", "color"]);
  for (const name of Object.keys(geometry.attributes)) {
    if (keep.has(name)) continue;
    geometry.deleteAttribute(name);
  }
  const uv = geometry.getAttribute("uv");
  if (uv && uv.itemSize === 4) {
    const compact = new THREE.Float32BufferAttribute(
      Array.from({ length: uv.count * 2 }, (_, index) =>
        index % 2 === 0 ? uv.getX(index >> 1) : uv.getY(index >> 1),
      ),
      2,
    );
    geometry.setAttribute("uv", compact);
  }
  geometry.computeBoundingSphere();
}

/** 深拷贝场景并统一替换/装配材质（不触碰编辑器视口对象）。 */
async function buildExportScene(
  payload: LotModelPayload,
  mode: ModelExportMode,
): Promise<THREE.Group> {
  const objects = await parseLotModelObjects(payload.glbs);
  const root = new THREE.Group();
  root.name = "openscp-export";

  if (mode === "white") {
    const white = new THREE.MeshStandardMaterial({
      color: 0xb8c2cc,
      roughness: 0.55,
      metalness: 0.12,
      side: THREE.DoubleSide,
    });
    for (const object of objects) {
      const clone = object.clone(true);
      clone.traverse((child) => {
        const mesh = child as THREE.Mesh;
        if (mesh.isMesh) {
          sanitizeGeometry(mesh.geometry);
          mesh.material = white;
        }
      });
      root.add(clone);
    }
    return root;
  }

  // textured：逐材质装配标准 PBR 贴图；tint facade 回退 COLOR_0 顶点色
  const materialCache = new Map<number, THREE.MeshStandardMaterial>();
  const pending: Promise<void>[] = [];
  for (const [index, material] of payload.materials.entries()) {
    const uvKind = payload.meshUvKinds[index] ?? 0;
    const standard = new THREE.MeshStandardMaterial({
      vertexColors: true,
      roughness: 0.82,
      metalness: 0,
      side: THREE.DoubleSide,
    });
    materialCache.set(index, standard);
    if (uvKind !== 1) continue;
    pending.push(
      (async () => {
        const [map, normalMap, roughnessMap, aoMap] = await Promise.all([
          material.baseColorPng ? loadTexture(material.baseColorPng) : null,
          material.normalPng ? loadTexture(material.normalPng) : null,
          material.roughnessPng ? loadTexture(material.roughnessPng) : null,
          material.aoPng ? loadTexture(material.aoPng) : null,
        ]);
        if (map) {
          map.colorSpace = THREE.SRGBColorSpace;
          standard.map = map;
        }
        if (normalMap) standard.normalMap = normalMap;
        if (roughnessMap) {
          standard.roughnessMap = roughnessMap;
          standard.roughness = 1;
        }
        if (aoMap) standard.aoMap = aoMap;
        standard.needsUpdate = true;
      })(),
    );
  }
  await Promise.all(pending);
  for (const [index, object] of objects.entries()) {
    const materialIndex = payload.meshMaterialIndices[index] ?? 0;
    const clone = object.clone(true);
    clone.traverse((child) => {
      const mesh = child as THREE.Mesh;
      if (mesh.isMesh) {
        sanitizeGeometry(mesh.geometry);
        mesh.material =
          materialCache.get(materialIndex) ??
          materialCache.get(0) ??
          new THREE.MeshStandardMaterial({ side: THREE.DoubleSide });
      }
    });
    root.add(clone);
  }
  return root;
}

export async function exportLotModel(options: {
  packageId: number;
  modelTgi: { typeId: number; group: number; instance: number };
  mode: ModelExportMode;
  defaultName: string;
}): Promise<void> {
  const toast = useToast();
  const { GLTFExporter } = await import(
    "three/examples/jsm/exporters/GLTFExporter.js"
  );
  const { parseLotModelContainer } = await import("@/lib/three-gltf");
  try {
    const buffer = await tauriApi.packages.readLotModelMeshes(
      options.packageId,
      options.modelTgi,
    );
    const payload = parseLotModelContainer(buffer);
    if (!payload.glbs.length) {
      toast.error("模型无可导出网格");
      return;
    }
    const scene = await buildExportScene(payload, options.mode);
    const glb = await new Promise<ArrayBuffer>((resolve, reject) => {
      new GLTFExporter().parse(
        scene,
        (result) => resolve(result as ArrayBuffer),
        (error) => reject(error),
        { binary: true },
      );
    });
    const path = await tauriApi.packages.saveFile(
      `${options.defaultName}-${options.mode}.glb`,
      "glb",
    );
    if (!path) return;
    // base64 编码（512MB 上限见 write_export_file）
    let binary = "";
    const bytes = new Uint8Array(glb);
    const chunk = 0x8000;
    for (let i = 0; i < bytes.length; i += chunk) {
      binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    await command("write_export_file", {
      request: { path, dataBase64: btoa(binary) },
    });
    toast.success(`已导出 ${options.mode === "white" ? "白模" : "带贴图模型"} → ${path}`);
  } catch (error) {
    toast.error(error instanceof Error ? error.message : "模型导出失败");
  }
}
