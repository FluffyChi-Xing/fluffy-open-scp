import * as THREE from "three";
import { command, type LotEditorSession } from "@/api/tauri";
import { createDataSource } from "@/api/data-source";
import { parseLotModelContainer, parseLotModelObjects } from "@/lib/three-gltf";
import { disposeObject } from "@/lib/three-viewer";
import { executeModel } from "@/render/exec/three/model";
import { stageGround } from "@/render/session/groundStage";
import {
  applySunEnv,
  createSunEnv,
} from "@/pages/packages/components/property-editor/refinedRender";
import { renderRefinedAssetUnit } from "@/pages/packages/components/property-editor/renderers";
import {
  draftPropResourceIds,
  loadDraftPropModels,
} from "../asset/workbenchPropModels";

const FIRE_STATION = {
  typeId: 0x00b1b104,
  group: 0x40e1c000,
  instance: 0xe917279c,
};

function imageUrl(png: string | null) {
  return png ? `data:image/png;base64,${png}` : null;
}

async function loadImage(url: string | null) {
  if (!url) return null;
  const image = new Image();
  image.src = url;
  await image.decode();
  return image;
}

async function loadPixels(url: string | null) {
  const image = await loadImage(url);
  if (!image) return null;
  const canvas = document.createElement("canvas");
  canvas.width = image.width;
  canvas.height = image.height;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.drawImage(image, 0, 0);
  return context.getImageData(0, 0, image.width, image.height);
}

function readMask(raw: string | null, width: number, height: number) {
  if (!raw) return null;
  const bytes = atob(raw);
  if (bytes.length !== width * height * 4) return null;
  const pixels = Uint8ClampedArray.from(bytes, (byte) => byte.charCodeAt(0));
  return new ImageData(pixels, width, height);
}

function assembleGround(
  session: LotEditorSession,
  group: THREE.Group,
  renderer: THREE.WebGLRenderer,
  isStale: () => boolean,
) {
  const { lotModelBboxCenter } = session as LotEditorSession & {
    lotModelBboxCenter?: [number, number] | null;
  };
  const mask = imageUrl(session.lotMaskPng);
  const albedo = imageUrl(session.lotAlbedoPng);
  const surface = imageUrl(session.lotSurfacePng);
  return stageGround({
    THREE,
    renderMode: "refined",
    lotSize: session.lotSize,
    lotPlacement: session.lotPlacement,
    lotOverlayBoxOffset: session.lotOverlayBoxOffset,
    lotModelBboxCenter: lotModelBboxCenter ?? session.lotModelBBoxCenter ?? null,
    lotMaskPng: mask,
    lotAlbedoPng: albedo,
    lotNormalAtlasPng: imageUrl(session.lotNormalAtlasPng),
    lotSurfacePng: surface,
    lotMaskRawRgba: session.lotMaskRawRgba,
    lotTilePeriod: session.lotTilePeriod,
    lotColors: session.lotColors,
    lotBorderColors: session.lotBorderColors,
    lotBorderPatternIndices: session.lotBorderPatternIndices,
    lotBorderWidths: session.lotBorderWidths,
    lotColorHeights: session.lotColorHeights,
    lotBorderHeights: session.lotBorderHeights,
    lotBaseTile: session.lotBaseTile,
    groupLot: group,
    primeTexture: (texture) => {
      if (!isStale()) renderer.initTexture(texture);
    },
    loadSurfacePixels: () => loadPixels(surface),
    loadMaskImageDims: () => loadImage(mask ?? albedo),
    loadImageDataFromUrl: loadPixels,
    loadRawMaskPixels: (width, height) =>
      readMask(session.lotMaskRawRgba, width, height),
    isStale,
  });
}

export async function loadCameraFireStation(
  anchorPackageId: number | undefined,
  renderer: THREE.WebGLRenderer,
  world: THREE.Group,
  sun: THREE.DirectionalLight,
  isStale: () => boolean,
) {
  const source = createDataSource();
  const packageId = await command<number>("city_camera_preview_packages", {
    packageId: anchorPackageId,
  });
  if (isStale()) return null;
  const session = await source.readLotEditorSession(packageId, FIRE_STATION);
  if (isStale()) return null;
  const lod = session.modelLods.find((entry) => entry !== null);
  if (!lod) throw new Error("Fire station LOD unavailable");
  const payload = parseLotModelContainer(
    await source.readLotModelMeshes(lod.packageId, lod.tgi),
  );
  if (!payload.glbs.length)
    throw new Error("Fire station geometry unavailable");
  const modelObjects = await parseLotModelObjects(
    payload.glbs,
    payload.upAxis,
    payload.upAxisDeclared,
  );
  if (isStale()) {
    modelObjects.forEach(disposeObject);
    return null;
  }

  const model = new THREE.Group();
  const ground = new THREE.Group();
  const details = new THREE.Group();
  world.add(model, ground, details);
  const env = createSunEnv(THREE);
  applySunEnv(env, 12, true);
  sun.color.copy(env.sunColor.value);
  sun.position.copy(env.sunDir.value).multiplyScalar(110);
  const modelStage = executeModel({
    THREE,
    env,
    payload,
    modelObjects,
    renderMode: "refined",
    modelSource: "native",
    reliefEnabled: true,
    modelSelectable: false,
    modelTransform: null,
    modelOrigin: null,
    maxAnisotropy: renderer.capabilities.getMaxAnisotropy(),
    isStale,
    specUniforms: [],
    buildingMeshes: [],
    unitObjects: new Map(),
    modelGroup: model,
    setShadowsEnabled: (enabled) => {
      if (!isStale()) renderer.shadowMap.enabled = enabled;
    },
    applySun: () => {},
    invalidate: () => {},
  });
  const propStage = (async () => {
    const assets = await loadDraftPropModels(
      source,
      draftPropResourceIds(session.units),
    );
    if (isStale()) return;
    const context = {
      THREE,
      renderMode: "refined" as const,
      envRefs: env,
      simParts: [],
      treeModelPayloads: assets.treeModelPayloads,
      treeAtlasPng: assets.treeAtlasPng,
      treeIds: assets.propTreeIds,
      propModels: assets.propModels,
    };
    await Promise.all(
      session.units
        .filter((unit) => unit.kind === "prop")
        .map(async (unit) => {
          const object = await renderRefinedAssetUnit(context, unit);
          if (!object) return;
          if (isStale()) disposeObject(object);
          else details.add(object);
        }),
    );
  })();
  const [refinedModel] = await Promise.allSettled([
    modelStage,
    assembleGround(session, ground, renderer, isStale),
    propStage,
  ]);
  if (isStale()) {
    modelObjects.filter((object) => !object.parent).forEach(disposeObject);
    return null;
  }
  if (refinedModel.status === "rejected") throw refinedModel.reason;
  world.updateMatrixWorld(true);
  const bounds = new THREE.Box3().setFromObject(model);
  if (bounds.isEmpty()) throw new Error("Fire station bounds unavailable");
  const groundBounds = new THREE.Box3().setFromObject(ground);
  return {
    center: bounds.getCenter(new THREE.Vector3()),
    groundHeight: groundBounds.isEmpty() ? bounds.min.y : groundBounds.min.y,
  };
}
