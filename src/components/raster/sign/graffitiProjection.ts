import type * as Three from "three";
import type { DecalUnit, DecalUnitTexture } from "@/api/tauri";
import { decalPreviewPixels } from "@/lib/raster-editor/decalPreview";
import type { RasterDocument } from "@/lib/raster-editor/document";
import {
  applySunEnv,
  createSunEnv,
} from "@/pages/packages/components/property-editor/refinedRender";
import { buildDecodedDecalBody } from "@/render/exec/three/decalBody";
import { createDecalReceiverIndex } from "@/render/exec/three/decalReceiverIndex";

const projector: DecalUnit = {
  kind: "decal",
  index: 0,
  category: 1,
  // Local Z points into the wall. Local X compensates for the engine's U flip.
  transform: { matrix: [-1, 0, 0, 0, 1, 0, 0, 0, -1, 0, 0, 0.05] },
  scale: 1,
  depth: 0.2,
  materialData: null,
  fields: [],
};

export async function createGraffitiProjection(
  THREE: typeof Three,
  doc: RasterDocument,
  colors: readonly string[],
  aspect: number,
  anisotropy: number,
  isStale: () => boolean,
) {
  const env = createSunEnv(THREE);
  const scene = new THREE.Scene();
  const wallColor = new THREE.Color("#77746d");
  const wall = new THREE.Mesh(
    new THREE.PlaneGeometry(Math.max(8, aspect * 4), 8),
    new THREE.MeshBasicMaterial({ color: wallColor }),
  );
  scene.add(wall);
  wall.updateMatrixWorld(true);

  const pixels = decalPreviewPixels(doc);
  const raster = new THREE.DataTexture(pixels, doc.width, doc.height);
  raster.colorSpace = THREE.NoColorSpace;
  raster.magFilter = THREE.LinearFilter;
  raster.minFilter = THREE.LinearMipmapLinearFilter;
  raster.generateMipmaps = true;
  raster.anisotropy = anisotropy;
  raster.needsUpdate = true;

  const texture: DecalUnitTexture = {
    category: projector.category,
    index: projector.index,
    idInstance: 0,
    atlasInstance: null,
    aspectRatio: aspect,
    width: doc.width,
    height: doc.height,
    png: null,
    error: null,
    materialInstance: 0xe5390a98,
    shaderDefInstance: null,
    variant: null,
    quantized: false,
    colors: colors.map((hex) => {
      const { r, g, b } = new THREE.Color(hex);
      return [r, g, b, 0];
    }),
  };
  const receivers = createDecalReceiverIndex();
  const state = { projected: 0, fallback: 0, holeLightCount: 0, neonAnimated: false };
  function dispose() {
    const materials = new Set<Three.Material>();
    scene.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return;
      object.geometry.dispose();
      for (const material of [object.material].flat()) materials.add(material);
    });
    for (const material of materials) material.dispose();
    receivers.clear();
    raster.dispose();
  }

  try {
    const decal = await buildDecodedDecalBody({
      THREE, unit: projector, texture, decoded: raster, proxies: [wall],
      env, receiverIndex: receivers, engineMaterials: true, decalLight: null,
      powered: true, isStale, state,
    });
    if (!decal) {
      dispose();
      return null;
    }
    scene.add(decal);
    if (state.projected !== 1) throw new Error("Graffiti preview receiver was not hit");
  } catch (error) {
    dispose();
    throw error;
  }

  return {
    scene,
    updatePixels(document: RasterDocument) {
      if (document.width !== doc.width || document.height !== doc.height) return;
      pixels.set(decalPreviewPixels(document));
      raster.needsUpdate = true;
    },
    setTime(time: number) {
      applySunEnv(env, time, true);
      wall.material.color.copy(wallColor).multiplyScalar(env.nightBoost.value);
    },
    dispose,
  };
}
