import { afterEach, expect, it, vi } from "vitest";
import * as THREE from "three";
import { stageGround, type GroundStageInput } from "./groundStage";
const mocks = vi.hoisted(() => ({ textures: vi.fn(async () => {}) }));
vi.mock("../exec/three/groundTextures", () => ({ applyGroundTextures: mocks.textures }));
vi.mock("@/pages/packages/components/property-editor/refinedGround", () => ({ prepareGroundComposer: vi.fn() }));
afterEach(() => { vi.useRealTimers(); vi.clearAllMocks(); });

it("presents white ground without awaiting refined-only atlas decoding", async () => {
  vi.useFakeTimers();
  const atlas = vi.fn(() => new Promise<null>(() => {}));
  const raw = vi.fn();
  const input = {
    THREE, lotSize: [32, 48], renderMode: "default", lotMaskPng: "mask",
    groupLot: new THREE.Group(), primeTexture: vi.fn(), isStale: () => false,
    loadSurfacePixels: atlas, loadMaskImageDims: vi.fn(),
    loadImageDataFromUrl: atlas, loadRawMaskPixels: raw,
    lotColors: [], lotBorderPatternIndices: [], lotBorderWidths: [], lotBaseTile: 8,
  } as unknown as GroundStageInput;
  expect(await stageGround(input)).toBeInstanceOf(THREE.Group);
  expect(atlas).not.toHaveBeenCalled();
  expect(raw).not.toHaveBeenCalled();
  expect(mocks.textures).toHaveBeenCalledWith(expect.objectContaining({ refined: false, rawMask: null }));
  vi.clearAllTimers();
});
