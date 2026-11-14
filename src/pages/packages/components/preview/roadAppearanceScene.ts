import * as THREE from "three";
import {
  createNativeRoads,
  type NativeRoadSceneData,
} from "@/components/map/map-native-roads";
import type { Region3DData } from "@/lib/region-map";

export type RoadAppearance = NonNullable<Region3DData["roadAssets"]>;

export async function createRoadAppearanceScene(
  assets: RoadAppearance,
  instance: number,
  sample: "straight" | "curve" = "curve",
): Promise<THREE.Group> {
  const images = [
    ...Object.entries(assets.textures),
    ...Object.entries(assets.models).map(
      ([id, model]) => [`model:${id}`, model.diffusePngBase64] as const,
    ),
  ];
  const pixels = Object.fromEntries(
    await Promise.all(
      images.map(async ([id, png]) => {
        const image = new Image();
        image.src = `data:image/png;base64,${png}`;
        await image.decode();
        const canvas = document.createElement("canvas");
        canvas.width = image.width;
        canvas.height = image.height;
        const context = canvas.getContext("2d", { willReadFrequently: true });
        if (!context) throw new Error("road_appearance_canvas");
        context.drawImage(image, 0, 0);
        return [
          id,
          context.getImageData(0, 0, image.width, image.height),
        ] as const;
      }),
    ),
  );
  // A fixed curved sample exposes sweep direction, lateral offsets and repeated props.
  const data: NativeRoadSceneData = {
    originWorld: [-256, -256],
    size: 512,
    metersPerPixel: 1,
    roadAssets: assets,
    staticLayers: {
      sources: [
        {
          site: "0",
          origin: [0, 0],
          bounds: null,
          maps: [],
          curves: [
            {
              entryId: instance,
              path: 0,
              controls:
                sample === "straight"
                  ? [
                      [-64, 0, 0],
                      [-21, 0, 0],
                      [21, 0, 0],
                      [64, 0, 0],
                    ]
                  : [
                      [-64, -20, 0],
                      [-21, -20, 0],
                      [21, 20, 0],
                      [64, 20, 0],
                    ],
            },
          ],
        },
      ],
    },
  };
  return createNativeRoads(data, pixels, () => 0);
}

export function disposeRoadAppearance(group: THREE.Object3D) {
  const geometries = new Set<THREE.BufferGeometry>();
  const materials = new Set<THREE.Material>();
  const textures = new Set<THREE.Texture>();
  group.traverse((object) => {
    if (!(object instanceof THREE.Mesh)) return;
    geometries.add(object.geometry);
    for (const material of Array.isArray(object.material)
      ? object.material
      : [object.material]) {
      materials.add(material);
      if (material.map instanceof THREE.Texture) textures.add(material.map);
    }
    if (object instanceof THREE.InstancedMesh) object.dispose();
  });
  geometries.forEach((geometry) => geometry.dispose());
  materials.forEach((material) => material.dispose());
  textures.forEach((texture) => texture.dispose());
}
