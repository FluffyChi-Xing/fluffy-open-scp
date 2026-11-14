/**
 * 地面贴图执行器（three）：mask/albdeo 贴图选择、精细合成缓存、材质替换。
 *
 * 从 `editorGround.applyGroundMask` 迁出（行为不变）；像素合成纯核在
 * `resolve/groundCompose`，worker/DOM 宿主仍在 `refinedGround`（后续可再拆）。
 */
import type * as ThreeNamespace from "three";
import { renderTelemetry } from "@/lib/renderTelemetry";
import { composeRefinedGround } from "@/pages/packages/components/property-editor/refinedGround";

type GroundTextures = {
  albedo: ThreeNamespace.Texture;
  normal: ThreeNamespace.Texture;
};

/** 会话级缓存：compose 输入在编辑操作中不变，命中 = 零合成零解码。 */
const groundTextureCache = new Map<string, ThreeNamespace.Texture>();
const groundComposeCache = new Map<string, GroundTextures>();
const GROUND_CACHE_CAP = 2;

function disposeGround(textures: GroundTextures | undefined): void {
  textures?.albedo.dispose();
  textures?.normal.dispose();
}

function putGroundCache(key: string, texture: GroundTextures): void {
  if (groundComposeCache.has(key)) return;
  groundComposeCache.set(key, texture);
  if (groundComposeCache.size > GROUND_CACHE_CAP) {
    const oldest = groundComposeCache.keys().next().value as string | undefined;
    if (oldest !== undefined) {
      disposeGround(groundComposeCache.get(oldest));
      groundComposeCache.delete(oldest);
    }
  }
}

/** 会话销毁/更换时清缓存（dispose 全部贴图）。 */
export function releaseGroundTextureCache(): void {
  for (const texture of groundComposeCache.values()) disposeGround(texture);
  groundComposeCache.clear();
  for (const texture of groundTextureCache.values()) texture.dispose();
  groundTextureCache.clear();
}

function loadGroundTexture(
  THREE: typeof ThreeNamespace,
  url: string,
): Promise<ThreeNamespace.Texture> {
  const cached = groundTextureCache.get(url);
  if (cached) return Promise.resolve(cached);
  return new THREE.TextureLoader().loadAsync(url).then((texture) => {
    groundTextureCache.set(url, texture);
    return texture;
  });
}

/** 与旧 groundComposeKey 完全同口径（任何输入变化都必须 miss）。 */
function groundComposeKey(options: {
  maskPng: string | null;
  rawMaskKey: string | null;
  surfaceKey: string | null;
  normalAtlasKey: string | null;
  lotColors: [number, number, number, number][];
  lotBorderColors?: [number, number, number][];
  lotBorderPatternIndices?: number[];
  lotBorderWidths?: number[];
  lotColorHeights?: number[];
  lotBorderHeights?: number[];
  lotSize?: [number, number] | null;
  tilePeriod?: [number, number] | null;
  baseTileIndex: number;
}): string {
  const o = options;
  return JSON.stringify([
    o.maskPng,
    o.rawMaskKey,
    o.surfaceKey,
    o.normalAtlasKey,
    o.lotColors,
    o.lotBorderColors ?? null,
    o.lotBorderPatternIndices ?? null,
    o.lotBorderWidths ?? null,
    o.lotColorHeights ?? null,
    o.lotBorderHeights ?? null,
    o.lotSize,
    o.tilePeriod,
    o.baseTileIndex,
  ]);
}

function findFill(
  ground: ThreeNamespace.Object3D,
): ThreeNamespace.Mesh | undefined {
  return ground.children.find(
    (child) => (child as ThreeNamespace.Mesh).isMesh,
  ) as ThreeNamespace.Mesh | undefined;
}

/** LotMask/albdeo → 地面 fill 材质（默认纯色 / 精细合成）。 */
export async function applyGroundTextures(options: {
  THREE: typeof ThreeNamespace;
  ground: ThreeNamespace.Object3D;
  maskPng: string | null;
  albedoPng?: string | null;
  lotSize?: [number, number] | null;
  tilePeriod?: [number, number] | null;
  refined: boolean;
  lotColors: [number, number, number, number][];
  surface?: ImageData | null;
  baseTileIndex: number;
  normalAtlas?: ImageData | null;
  rawMask?: ImageData | null;
  lotBorderColors?: [number, number, number][];
  lotBorderPatternIndices?: number[];
  lotBorderWidths?: number[];
  lotColorHeights?: number[];
  lotBorderHeights?: number[];
  lotOverlayBoxOffset?: [number, number] | null;
  rawMaskKey?: string | null;
  surfaceKey?: string | null;
  normalAtlasKey?: string | null;
  isStale: () => boolean;
}): Promise<void> {
  const {
    THREE,
    ground,
    maskPng,
    albedoPng,
    lotSize,
    tilePeriod,
    refined,
    lotColors,
    surface,
    baseTileIndex,
    normalAtlas,
    rawMask,
    lotBorderColors,
    lotBorderPatternIndices,
    lotBorderWidths,
    isStale,
  } = options;
  const flatPng = refined ? maskPng : (albedoPng ?? maskPng);
  if (!flatPng) return;
  const texture = await loadGroundTexture(THREE, flatPng);
  if (isStale()) return;
  texture.colorSpace = THREE.SRGBColorSpace;
  // 原始栅格行序：不翻 V，否则遮罩南北镜像。
  texture.flipY = false;
  const fill = findFill(ground);
  if (!fill) return;
  const applyFlat = async () => {
    const fallback = refined && albedoPng
      ? await loadGroundTexture(THREE, albedoPng)
      : texture;
    if (isStale()) return;
    fallback.colorSpace = THREE.SRGBColorSpace;
    fallback.flipY = false;
    const material = fill.material as ThreeNamespace.MeshBasicMaterial;
    material.map = fallback;
    material.transparent = false;
    material.opacity = 1;
    material.color.set(0xffffff);
    material.needsUpdate = true;
  };
  if (refined && maskPng) {
    const span = renderTelemetry.begin("texture_compose", { phase: "ground" });
    try {
      const key = groundComposeKey({
        maskPng,
        rawMaskKey: options.rawMaskKey ?? null,
        surfaceKey: options.surfaceKey ?? null,
        normalAtlasKey: options.normalAtlasKey ?? null,
        lotColors,
        lotBorderColors,
        lotBorderPatternIndices,
        lotBorderWidths,
        lotColorHeights: options.lotColorHeights,
        lotBorderHeights: options.lotBorderHeights,
        lotSize,
        tilePeriod,
        baseTileIndex,
      });
      let result = groundComposeCache.get(key);
      const cacheHit = Boolean(result);
      if (!result) {
        const composed = await composeRefinedGround({
          THREE,
          maskImage: texture.image as TexImageSource,
          lotColors,
          lotSize: lotSize ?? null,
          tilePeriod: tilePeriod ?? null,
          surface: surface ?? null,
          baseTileIndex,
          normalAtlas: normalAtlas ?? null,
          rawMask: rawMask ?? null,
          lotBorderColors: lotBorderColors ?? null,
          lotBorderPatternIndices: lotBorderPatternIndices ?? null,
          lotBorderWidths: lotBorderWidths ?? null,
          lotColorHeights: options.lotColorHeights,
          lotBorderHeights: options.lotBorderHeights,
        });
        if (composed) {
          putGroundCache(key, composed);
          result = composed;
        }
      }
      if (isStale()) return;
      if (!result) {
        await applyFlat();
        span.end({ failed: true });
        return;
      }
      const fillMaterial = fill.material as ThreeNamespace.MeshBasicMaterial;
      const lit = new THREE.MeshPhongMaterial({
        map: result.albedo,
        normalMap: result.normal,
        normalScale: new THREE.Vector2(1.8, 1.8),
        shininess: 8,
        specular: 0x111111,
      });
      fill.receiveShadow = true;
      fillMaterial.dispose();
      fill.material = lit;
      span.end({ cacheHit });
    } catch {
      await applyFlat();
      span.end({ failed: true });
    }
  } else {
    await applyFlat();
  }
}
