/** Game placement semantics, NOT the renderer's unknown-material fallback.
 * Verified material compiled states and HLSL chains:
 * docs/re/decal-engine-reanalysis-ida.md sections 11.3-12.3.
 * 73684EFC: projected CF + free-standing D1; E5390A98: projected CD;
 * 4491DE3A: projected D2 with a second cube coordinate for the interior.
 * A float-quad VS alone does not imply a free-standing decal (D2 uses both).
 */
export type DecalPlacementKind = "projected" | "floating" | "unknown";

export function decalPlacementKind(material: number | null | undefined): DecalPlacementKind {
  switch (material) {
    case 0x73684efc: return "floating";
    case 0xe5390a98:
    case 0x4491de3a: return "projected";
    default: return "unknown";
  }
}
