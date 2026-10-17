import { describe, expect, it } from "vitest";
import type { Region3DData, RoadSweep, StateSource } from "@/lib/region-map";
import {
  createNativeRoads,
  createStaticRoads,
  curveChains,
  hasNativeRoadNetwork,
  roadUnitTransform,
} from "./map-native-roads";
import * as THREE from "three";

const pixel = {
  width: 1,
  height: 1,
  data: new Uint8ClampedArray([255, 255, 255, 255]),
} as ImageData;
const curve: StateSource["curves"][number] = {
  entryId: 1,
  path: 0,
  controls: [
    [0, 0, 20],
    [0, 1000 / 3, 20],
    [0, 2000 / 3, 20],
    [0, 1000, 20],
  ],
};
const data = {
  size: 128,
  metersPerPixel: 16,
  originWorld: [-1024, -1024],
  staticLayers: {
    sources: [
      { site: "0", origin: [0, 0], bounds: null, maps: [], curves: [curve] },
      {
        site: "1029",
        origin: [500, 0],
        bounds: [0, 0, 1000, 1000],
        maps: [],
        curves: [curve],
      },
    ],
  },
  roadAssets: {
    instances: [],
    unsupportedComponents: [],
    textures: {},
    ribbons: {
      1: [
        {
          component: 1,
          texture: 1,
          offset: [0, 0, 0],
          scale: [1, 21, 1],
          worldSize: [4, 8],
          uvStart: [0, 0],
          uvEnd: [1, 1],
        },
      ],
    },
    sweeps: {
      1: [
        {
          model: 2,
          instance: true,
          scale: [1.3, 1, 1],
          rotation: [0, 0, 0],
          offset: [0, 0, 0],
          interval: 500,
          start: 250,
          end: 0,
          length: 0,
          step: 0,
          distort: true,
          repeatUv: false,
          roundIntervals: true,
          relativeToGround: false,
          stackToGround: false,
        } satisfies RoadSweep,
      ],
    },
    models: {
      2: {
        id: 2,
        positions: [-1, 0, 0, 1, 0, 0, 0, 1, 10],
        normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
        uvs: [0, 0, 1, 0, 0, 1],
        indices: [0, 1, 2],
        diffusePngBase64: "",
      },
    },
  },
} as unknown as Region3DData;

describe("native empty-map geometry", () => {
  it("maps CapeTrinity unit Y axis into the world road direction", () => {
    const matrix = roadUnitTransform(
      [0.006266865, 0.9999804, 0, -0.9999804, 0.006266865, 0, 0, 0, 1],
      1,
      [981.13574, 2902.8171, -851.5662],
    );
    const endpoint = new THREE.Vector3(0, -100, 1).applyMatrix4(matrix);
    expect(endpoint.x).toBeCloseTo(1081.13, 1);
    expect(endpoint.z).toBeCloseTo(2902.19, 1);
  });

  it("splits disconnected branches while preserving small EGB endpoint error", () => {
    const records: StateSource["curves"] = [
      curve,
      {
        ...curve,
        controls: curve.controls.map(
          (p) => [p[0] + 100, p[1], p[2]] as [number, number, number],
        ),
      },
      {
        ...curve,
        controls: curve.controls.map(
          (p) => [p[0], p[1] + 1000.1, p[2]] as [number, number, number],
        ),
      },
    ];
    const chains = curveChains(records);
    expect(chains).toHaveLength(2);
    expect(chains.map((chain) => chain.length).sort()).toEqual([1, 2]);
  });

  it("prefers region-zero curves over the ED fallback", () => {
    expect(hasNativeRoadNetwork(data)).toBe(true);
    expect(
      hasNativeRoadNetwork({
        staticLayers: {
          sources: [
            { site: "0", origin: [0, 0], bounds: null, maps: [], curves: [] },
          ],
        },
      } as unknown as Region3DData),
    ).toBe(false);
  });

  it("keeps railway repetition in metres and excludes played-city copies", () => {
    const group = createNativeRoads(
      data,
      { 1: pixel, "model:2": pixel },
      () => 0,
    );
    try {
      const ribbon = group.children[0] as THREE.Mesh<THREE.BufferGeometry>;
      const uv = ribbon.geometry.getAttribute("uv"),
        position = ribbon.geometry.getAttribute("position");
      expect(uv.getY(uv.count - 1)).toBeCloseTo(-125);
      ribbon.geometry.computeBoundingBox();
      expect(ribbon.geometry.boundingBox!.max.x).toBe(2);
      expect(position.count).toBe(500);
      const towers = group.children[1] as THREE.Mesh<THREE.BufferGeometry>;
      towers.geometry.computeBoundingBox();
      expect(towers.geometry.boundingBox!.max.y).toBeCloseTo(33);
      expect(towers.geometry.getAttribute("position").count).toBe(6);
    } finally {
      for (const child of group.children) {
        const mesh = child as THREE.Mesh<THREE.BufferGeometry, THREE.Material>;
        mesh.geometry.dispose();
        mesh.material.dispose();
      }
    }
  });
});

describe("static region road geometry", () => {
  it("renders ED road-control cells without save layers", () => {
    const size = 16;
    const pixels = new Uint8ClampedArray(size * size * 4).fill(255);
    for (let y = 0; y < size; y++) {
      const i = (y * size + 8) * 4;
      pixels[i + 1] = 0;
      pixels[i + 2] = 0;
    }
    const group = createStaticRoads(
      { size, metersPerPixel: 16, originWorld: [-128, -128] } as Region3DData,
      { width: size, height: size, data: pixels } as ImageData,
      () => 0,
    );
    expect(group.children).toHaveLength(1);
    expect(
      (group.children[0] as THREE.Mesh).geometry.getAttribute("position").count,
    ).toBeGreaterThan(0);
  });
});
