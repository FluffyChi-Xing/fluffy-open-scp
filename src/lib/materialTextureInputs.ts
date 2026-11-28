import type { Material, Texture } from "three";

const inputs = Symbol("renderTextureInputs");
type RegisteredMaterial = Material & { [inputs]?: readonly Texture[] };
/** Non-enumerable metadata: never serialized by Material.clone/userData or sent to a shader. */
export function registerTextureUniforms(
  material: Material,
  uniforms: Record<string, { value: unknown }>,
): void {
  const textures = Object.values(uniforms)
    .map((u) => u.value)
    .filter((v): v is Texture => Boolean((v as Texture | null)?.isTexture));
  Object.defineProperty(material, inputs, {
    value: textures,
    configurable: true,
  });
}
export function registeredTextureInputs(
  material: Material,
): readonly Texture[] {
  return (material as RegisteredMaterial)[inputs] ?? [];
}
