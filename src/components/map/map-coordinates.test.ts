import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { createMapSceneRoot, mapToScene, sceneToMap } from "./map-coordinates";
import { fieldIndex } from "./map-fields";

describe("map coordinate handedness", () => {
  it("preserves the oriented game basis and distance instead of reflecting it", () => {
    const x = mapToScene(1, 0, 0), y = mapToScene(0, 1, 0), up = mapToScene(0, 0, 1);
    expect(x.clone().cross(y).distanceTo(up)).toBeLessThan(1e-10);
    const p = mapToScene(140, -390, 25);
    expect(sceneToMap(p).toArray()).toEqual([140, -390, 25]);
    expect(p.length()).toBeCloseTo(Math.hypot(140, -390, 25));
  });

  it("keeps asymmetric terrain, road and city anchors registered in one map root", () => {
    const root = createMapSceneRoot();
    for (const [x, y, height] of [[-40, 120, -850], [140, -80, -820], [30, 60, -870]]) {
      const layer = new THREE.Group();
      layer.position.set(x, height, y);
      root.add(layer);
      root.updateMatrixWorld(true);
      const position = layer.getWorldPosition(new THREE.Vector3());
      expect(position.distanceTo(mapToScene(x, y, height))).toBeLessThan(1e-10);
      const game = sceneToMap(position);
      expect(fieldIndex(game.x, game.y, 32, [-256, -256], 16))
        .toBe(fieldIndex(x, y, 32, [-256, -256], 16));
    }
  });

  it("shows positive game X on the right when viewing north (+Y), with north upwards", () => {
    const camera = new THREE.PerspectiveCamera(45, 1, 1, 10000);
    camera.position.copy(mapToScene(0, -500, 800));
    camera.lookAt(mapToScene(0, 0, 0));
    camera.updateMatrixWorld();
    const east = mapToScene(100, 0, 0).project(camera);
    const north = mapToScene(0, 100, 0).project(camera);
    expect(east.x).toBeGreaterThan(0);
    expect(north.y).toBeGreaterThan(0);
  });
});
