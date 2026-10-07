/** MaterialInfo + shader fragments, see docs/re/property-rendering-audit.md.
 * Animation speed is a uniform, never a geometry selector.
 */
export function decalRoute(material: number | null | undefined) {
  switch ((material ?? 0) >>> 0) {
    case 0x73684efc:
      return { family: "sdf", surface: "plane" } as const;
    case 0xe5390a98:
      return { family: "sign", surface: "projected" } as const;
    case 0x4491de3a:
      return { family: "hole", surface: "projected" } as const;
    default:
      return { family: "clip", surface: "projected" } as const;
  }
}
