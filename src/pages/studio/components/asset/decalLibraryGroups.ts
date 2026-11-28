import type { Tgi } from "@/api/tauri";

export interface DecalPackage {
  packageId: number;
  path: string;
  atlases: { tgi: Tgi; entryCount: number }[];
}

/** The atlas group is the category; package handles are only source locations. */
export function decalLibraryGroups(packages: readonly DecalPackage[]) {
  const groups = new Map<number, { id: number; atlases: (DecalPackage["atlases"][number] & { packageId: number; path: string })[] }>();
  const seen = new Set<string>();
  for (const source of [...packages].sort((a, b) => a.packageId - b.packageId)) {
    for (const atlas of source.atlases) {
      const key = `${atlas.tgi.typeId}:${atlas.tgi.group}:${atlas.tgi.instance}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const group = groups.get(atlas.tgi.group) ?? { id: atlas.tgi.group, atlases: [] };
      group.atlases.push({ ...atlas, packageId: source.packageId, path: source.path });
      groups.set(group.id, group);
    }
  }
  return [...groups.values()].sort((a, b) => a.id - b.id);
}
