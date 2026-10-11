import type { Tgi } from "@/api/tauri";
export interface RasterCatalogEntry {
  packageId: number;
  tgi: Tgi;
  ownerPackageId: number;
  owner: Tgi;
  entryIndex: number | null;
}
export const catalogKey = (entry: RasterCatalogEntry) =>
  `${entry.packageId}:${entry.tgi.typeId}:${entry.tgi.group}:${entry.tgi.instance}:${entry.ownerPackageId}:${entry.owner.instance}:${entry.entryIndex ?? ""}`;
export function matchesDecalProfile(profile: string, material: number | null) {
  if (profile === "decal-rgba") return true;
  if (profile === "decal-sign" || profile === "decal-sdf")
    return material === 0x73684efc;
  if (profile === "decal-graffiti") return material === 0xe5390a98;
  return profile === "decal-interior" && material === 0x4491de3a;
}
