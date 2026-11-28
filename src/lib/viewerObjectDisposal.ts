import type { Material, Mesh, Object3D } from "three";
import { isSharedGeometry } from "./three-gltf";

/** Shared cached geometry belongs to its cache, per-scene materials to the view. */
export function disposeObject(object: Object3D): void {
  object.traverse((child) => {
    const mesh = child as Mesh & { isLine?: boolean };
    if (!mesh.isMesh && !mesh.isLine) return;
    if (!isSharedGeometry(mesh.geometry)) mesh.geometry?.dispose();
    const material: Material | Material[] = mesh.material;
    if (Array.isArray(material)) material.forEach((item) => item.dispose());
    else material?.dispose();
  });
}
