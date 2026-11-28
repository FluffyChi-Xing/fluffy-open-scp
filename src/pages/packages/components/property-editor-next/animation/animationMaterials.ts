import * as THREE from "three";
import type { LotModelPayload } from "@/api/tauri";
import { pngBlobUrl } from "@/lib/three-gltf";
import { createSunEnv, loadTintTextures, makeTintMaterial } from "../../property-editor/refinedRender";

export async function applyAnimationMaterials(
  roots: THREE.Object3D[],
  payload: LotModelPayload,
) {
  if (roots.length !== payload.glbs.length) return () => {};
  const urls: string[] = [];
  const textures = new Set<THREE.Texture>();
  const dispose = () => {
    for (const url of urls) URL.revokeObjectURL(url);
    for (const texture of textures) {
      if (typeof ImageBitmap !== "undefined" && texture.image instanceof ImageBitmap) {
        texture.image.close();
      }
      texture.dispose();
    }
  };
  try {
    const env = createSunEnv(THREE);
    const sets = await loadTintTextures(THREE, payload.materials, 8, urls);
    for (const set of sets) {
      for (const texture of [set.tintTex, set.paletteTex, set.normalTex, set.shaderTex,
        set.interiorTex, set.paramsTex, ...set.paramsByMaterial.values()]) {
        if (texture) textures.add(texture);
      }
    }
    const loader = new THREE.TextureLoader();
    const load = async (bytes: Parameters<typeof pngBlobUrl>[0] | null, srgb = false) => {
      if (!bytes) return null;
      const url = pngBlobUrl(bytes);
      urls.push(url);
      const texture = await loader.loadAsync(url).catch(() => null);
      if (!texture) return null;
      texture.wrapS = texture.wrapT = THREE.RepeatWrapping;
      if (srgb) texture.colorSpace = THREE.SRGBColorSpace;
      textures.add(texture);
      return texture;
    };
    const mapsByMaterial = new Map<number, ReturnType<typeof loadMaps>>();
    async function loadMaps(index: number) {
      const data = payload.materials[index];
      return Promise.all([
        load(data?.baseColorPng ?? null, true), load(data?.normalPng ?? null),
        load(data?.roughnessPng ?? null), load(data?.aoPng ?? null),
      ]);
    }
    for (const [i, root] of roots.entries()) {
      const index = payload.meshMaterialIndices[i] ?? 0;
      const tint = sets[index];
      const useTint = payload.meshUvKinds?.[i] === 2 && tint?.tintTex && tint.paletteTex;
      if (!useTint && !mapsByMaterial.has(index)) mapsByMaterial.set(index, loadMaps(index));
      const maps = useTint ? null : await mapsByMaterial.get(index);
      root.traverse((object) => {
        const mesh = object as THREE.Mesh;
        if (!mesh.isMesh || Array.isArray(mesh.material)) return;
        if (useTint && mesh.geometry.attributes.uv) {
          const uv = mesh.geometry.attributes.uv;
          for (let v = 0; v < uv.count; v++) uv.setY(v, -uv.getY(v));
          uv.needsUpdate = true;
          mesh.material.dispose();
          [mesh.material] = makeTintMaterial(THREE, tint, env);
        } else if (maps?.some(Boolean)) {
          const [map, normalMap, roughnessMap, aoMap] = maps;
          mesh.material.dispose();
          mesh.material = new THREE.MeshStandardMaterial({
            map, normalMap, roughnessMap, aoMap,
            roughness: roughnessMap ? 1 : 0.8,
            side: THREE.DoubleSide,
          });
        }
      });
    }
    return dispose;
  } catch (error) {
    dispose();
    throw error;
  }
}
