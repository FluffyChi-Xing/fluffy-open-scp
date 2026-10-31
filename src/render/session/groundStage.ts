/**
 * 地面阶段（session）：地面矩形 + placement 逆 + overlay 偏移 + mask 合成。
 *
 * 与旧 `assembleScene.groundStage` 同序同参；贴图解码/合成仍走既有
 * `applyGroundMask`（后续再迁资源层），本阶段只负责编排与几何。
 */
import type * as ThreeNamespace from "three";
import { renderTelemetry } from "@/lib/renderTelemetry";
import { prepareGroundComposer } from "@/pages/packages/components/property-editor/refinedGround";
import {
  groundFillMesh,
} from "@/pages/packages/components/property-editor/editorGround";
import { applyGroundTextures } from "../exec/three/groundTextures";
import { createLotGround } from "../exec/three/ground";
import { lotGroundPlan } from "../plan/ground";

type GroundMaskInput = Parameters<typeof applyGroundTextures>[0];

export interface GroundStageInput {
  readonly THREE: typeof ThreeNamespace;
  readonly lotSize: [number, number] | null;
  readonly lotPlacement: number[] | null;
  readonly lotOverlayBoxOffset: [number, number] | null;
  readonly lotModelBboxCenter: [number, number] | null;
  readonly renderMode: "default" | "refined";
  readonly lotMaskPng: string | null;
  readonly lotAlbedoPng: string | null;
  readonly lotNormalAtlasPng: string | null;
  readonly lotSurfacePng: string | null;
  readonly lotMaskRawRgba: string | null;
  readonly lotTilePeriod: [number, number] | null;
  readonly lotColors: GroundMaskInput["lotColors"];
  readonly lotBorderColors: GroundMaskInput["lotBorderColors"];
  readonly lotBorderPatternIndices: number[];
  readonly lotBorderWidths: number[];
  readonly lotColorHeights?: number[];
  readonly lotBorderHeights?: number[];
  readonly lotBaseTile: number;
  readonly groupLot: ThreeNamespace.Object3D;
  readonly primeTexture: (texture: ThreeNamespace.Texture) => void;
  readonly loadSurfacePixels: () => Promise<GroundMaskInput["surface"]>;
  readonly loadMaskImageDims: () => Promise<{ width: number; height: number } | null>;
  readonly loadImageDataFromUrl: (
    url: string | null,
  ) => Promise<GroundMaskInput["normalAtlas"]>;
  readonly loadRawMaskPixels: (
    width: number,
    height: number,
  ) => GroundMaskInput["rawMask"];
  readonly isStale: () => boolean;
}

export async function stageGround(
  input: GroundStageInput,
): Promise<ThreeNamespace.Object3D | null> {
  if (!input.lotSize) return null;
  if (input.renderMode === "refined") prepareGroundComposer();
  const span = renderTelemetry.begin("lot_render", {
    refined: input.renderMode === "refined",
  });
  const plan = lotGroundPlan(input.lotSize, input.lotPlacement);
  const ground = plan
    ? createLotGround(input.THREE, plan)
    : createLotGround(input.THREE, {
        width: 0,
        depth: 0,
        corners: [
          [0, 0, 0.05],
          [0, 0, 0.05],
          [0, 0, 0.05],
          [0, 0, 0.05],
        ],
        fillZ: 0.02,
        placementInverse: null,
      });
  // 关闭自动更新：矩阵完全由 placement 逆决定，防止渲染循环覆盖。
  // cZoningGame::CreateUnitLotGraphics (0x8BA1C0) / cRegionCityLots (0x86DCB0)
  // 使用 authored offset（显式 0 也算），缺失才回退 bbox 中心；在 lot 局部
  // 坐标、逆 placement 之前应用：P^-1 * T(c)。
  const centerLocal = input.lotOverlayBoxOffset ??
    input.lotModelBboxCenter ?? [0, 0];
  if (Math.abs(centerLocal[0]) > 1e-4 || Math.abs(centerLocal[1]) > 1e-4) {
    ground.matrix.multiply(
      new input.THREE.Matrix4().makeTranslation(centerLocal[0], centerLocal[1], 0),
    );
  }
  ground.matrixAutoUpdate = false;
  input.groupLot.add(ground);

  if (input.lotMaskPng || input.lotAlbedoPng) {
    const [surface, maskDims, normalAtlas] = await Promise.all([
      input.loadSurfacePixels(),
      input.loadMaskImageDims(),
      input.loadImageDataFromUrl(input.lotNormalAtlasPng),
    ]);
    if (!surface) {
      // 精细渲染的底图格依赖真实图集；静默占位会把沥青画成亮灰。
      console.warn(
        "[lot-ground] 'Lot Textures' surface unavailable — refined ground will use placeholder tiles. Open SimCity_Graphics.package (and the lot's own package) for the real atlas.",
      );
    }
    const rawMask = maskDims
      ? input.loadRawMaskPixels(maskDims.width, maskDims.height)
      : null;
    await applyGroundTextures({
      rawMask,
      surface,
      normalAtlas,
      THREE: input.THREE,
      ground,
      maskPng: input.lotMaskPng,
      albedoPng: input.lotAlbedoPng,
      lotSize: input.lotSize,
      tilePeriod: input.lotTilePeriod,
      refined: input.renderMode === "refined",
      lotColors: input.lotColors,
      lotBorderColors: input.lotBorderColors,
      lotBorderPatternIndices: input.lotBorderPatternIndices,
      lotBorderWidths: input.lotBorderWidths,
      lotColorHeights: input.lotColorHeights,
      lotBorderHeights: input.lotBorderHeights,
      baseTileIndex: input.lotBaseTile,
      lotOverlayBoxOffset: input.lotOverlayBoxOffset,
      rawMaskKey: input.lotMaskRawRgba,
      surfaceKey: input.lotSurfacePng,
      normalAtlasKey: input.lotNormalAtlasPng,
      isStale: input.isStale,
    });
  }
  span.end({ masked: Boolean(input.lotMaskPng || input.lotAlbedoPng) });

  // 合成产物提前上传 GPU：2048² + mipmap 上传是首帧冻结源，放在地面线内
  // 与模型/单元线并发吸收。
  const fill = groundFillMesh(ground);
  const fillMap = (fill?.material as ThreeNamespace.Material | undefined)
    ? ((fill!.material as ThreeNamespace.MeshLambertMaterial).map ?? null)
    : null;
  if (fillMap) input.primeTexture(fillMap);
  return ground;
}
