import { describe, expect, it } from "vitest";
import type { Region3DData, RoadSweep } from "@/lib/region-map";
import { createNativeRoads } from "./map-native-roads";
import * as THREE from "three";

const pixel = {
  width: 1,
  height: 1,
  data: new Uint8ClampedArray([255, 255, 255, 255]),
} as ImageData;
const curve = {
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
  saveLayers: {
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
