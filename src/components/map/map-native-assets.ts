import * as THREE from "three";
import type { MapModel } from "@/lib/region-map";

export function nativeTexture(pixels: ImageData): THREE.DataTexture {
  const t = new THREE.DataTexture(
    new Uint8Array(pixels.data),
    pixels.width,
    pixels.height,
  );
  t.colorSpace = THREE.SRGBColorSpace;
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.magFilter = THREE.LinearFilter;
  t.minFilter = THREE.LinearMipmapLinearFilter;
  t.generateMipmaps = true;
  t.anisotropy = 4;
  t.needsUpdate = true;
  return t;
}
export function nativeGeometry(model: MapModel): THREE.BufferGeometry {
  const g = new THREE.BufferGeometry();
  // Keep game-local xyz here. Consumers decide whether to rotate or sweep.
  g.setAttribute(
    "position",
    new THREE.Float32BufferAttribute(model.positions, 3),
  );
  g.setAttribute("normal", new THREE.Float32BufferAttribute(model.normals, 3));
  g.setAttribute("uv", new THREE.Float32BufferAttribute(model.uvs, 2));
  g.setIndex(model.indices);
  g.computeBoundingBox();
  return g;
}

export interface TreeAsset {
  geometry: THREE.BufferGeometry;
  material: THREE.MeshLambertMaterial;
  farGeometry: THREE.BufferGeometry;
  farMaterial: THREE.MeshBasicMaterial;
}

/** Bake a local impostor from the actual mesh. Does not distribute copyrighted atlases. */
export function nativeTreeAssets(
  models: MapModel[],
  pixels: ImageData[],
  renderer: THREE.WebGLRenderer,
): TreeAsset[] {
  return models.map((model, i) => {
    const geometry = nativeGeometry(model);
    geometry.rotateX(-Math.PI / 2); // game xyz -> Three x,z,-y for atlas and local tree axes
    geometry.computeBoundingBox();
    const box = geometry.boundingBox!,
      size = box.getSize(new THREE.Vector3());
    const material = new THREE.MeshLambertMaterial({
      map: nativeTexture(pixels[i]),
      alphaTest: 0.5,
      side: THREE.DoubleSide,
    });
    const bake = new THREE.Scene();
    bake.add(
      new THREE.Mesh(geometry, material),
      new THREE.AmbientLight(0xffffff, 1.2),
    );
    const sun = new THREE.DirectionalLight(0xffffff, 1.3);
    sun.position.set(1, 2, 1);
    bake.add(sun);
    const width = Math.max(size.x, size.z) * 1.45,
      height = size.y * 1.05;
    const center = box.getCenter(new THREE.Vector3());
    const camera = new THREE.OrthographicCamera(
      -width / 2,
      width / 2,
      height / 2,
      -height / 2,
      0.1,
      1000,
    );
    camera.position.copy(center).add(new THREE.Vector3(100, 0, 100));
    camera.lookAt(center);
    const target = new THREE.WebGLRenderTarget(256, 256, { depthBuffer: true });
    const previous = renderer.getRenderTarget(),
      clear = renderer.getClearColor(new THREE.Color()),
      alpha = renderer.getClearAlpha();
    try {
      renderer.setRenderTarget(target);
      renderer.setClearColor(0, 0);
      renderer.clear();
      renderer.render(bake, camera);
    } finally {
      renderer.setRenderTarget(previous);
      renderer.setClearColor(clear, alpha);
    }
    const rgba = new Uint8Array(256 * 256 * 4);
    renderer.readRenderTargetPixels(target, 0, 0, 256, 256, rgba);
    const atlas = new THREE.DataTexture(rgba, 256, 256);
    atlas.colorSpace = THREE.LinearSRGBColorSpace;
    atlas.magFilter = THREE.LinearFilter;
    atlas.minFilter = THREE.LinearMipmapLinearFilter;
    atlas.generateMipmaps = true;
    atlas.needsUpdate = true;
    target.dispose();
    const farGeometry = new THREE.PlaneGeometry(width, height);
    farGeometry.translate(0, center.y, 0);
    // Map root handles the final -Y boundary; keep near geometry map-local.
    geometry.scale(1, 1, -1);
    const index = geometry.getIndex()!;
    for (let j = 0; j < index.count; j += 3) {
      const a = index.getX(j);
      index.setX(j, index.getX(j + 2));
      index.setX(j + 2, a);
    }
    const farMaterial = new THREE.MeshBasicMaterial({
      map: atlas,
      alphaTest: 0.35,
      side: THREE.DoubleSide,
    });
    farMaterial.onBeforeCompile = (shader) => {
      shader.vertexShader = shader.vertexShader.replace(
        "#include <project_vertex>",
        `
        vec4 mvPosition = modelViewMatrix * instanceMatrix * vec4(0.0,0.0,0.0,1.0);
        mvPosition.xy += position.xy * length(instanceMatrix[0].xyz);
        gl_Position = projectionMatrix * mvPosition;
      `,
      );
    };
    farMaterial.customProgramCacheKey = () => "map-forest-impostor-v1";
    material.addEventListener("dispose", () => material.map?.dispose());
    farMaterial.addEventListener("dispose", () => atlas.dispose());
    return { geometry, material, farGeometry, farMaterial };
  });
}
