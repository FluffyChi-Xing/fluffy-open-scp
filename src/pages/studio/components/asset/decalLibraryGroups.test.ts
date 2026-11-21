import { expect, it } from "vitest";
import { decalLibraryGroups } from "./decalLibraryGroups";

it("groups by full atlas group and deduplicates repeated package handles and atlas TGIs", () => {
  const a = { tgi: { typeId: 1, group: 0xc67fb185, instance: 8 }, entryCount: 29 };
  const b = { tgi: { typeId: 1, group: 0xfb661651, instance: 9 }, entryCount: 385 };
  const result = decalLibraryGroups([
    { packageId: 3, path: "Game.package", atlases: [a, b] },
    { packageId: 6, path: "\\\\?\\Game.package", atlases: [a, b] },
    { packageId: 7, path: "other.package", atlases: [a] },
  ]);
  expect(result.map((group) => [group.id, group.atlases.length])).toEqual([[a.tgi.group, 1], [b.tgi.group, 1]]);
  expect(result[0].atlases[0].packageId).toBe(3);
});
