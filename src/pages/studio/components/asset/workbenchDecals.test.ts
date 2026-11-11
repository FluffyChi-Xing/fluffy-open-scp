import { expect, it, vi } from "vitest";
import { command } from "@/api/tauri";
import type { DecalUnit, DecalUnitTexture } from "@/api/tauri";
import { schemaUnitJson, unitsFromSchema } from "@/pages/packages/components/property-editor/peSchemaDoc";
import { createPlacedDecal, restoreDecalSources, useWorkbenchDecals, writeDecalSources } from "./workbenchDecals";

vi.mock("@/api/tauri", () => ({ command: vi.fn() }));

const id = { typeId: 0, group: 0, instance: 0xd2dd48c5 };
const source = {
  packagePath: "D:/Game/SimCity_Game.package",
  atlas: { typeId: 0x00b1b104, group: 0xc67fb185, instance: 0xeefd390c },
  rasterPackagePath: "D:/Game/SimCity_Graphics.package",
};
const placement = {
  category: 1,
  basis: [1, 0, 0, 0, 1, 0, 0, 0, 1],
  scale: 1.25,
  depth: 2,
  materialData: [0.5, 0, 0] as [number, number, number],
};

it("preserves the drop transform, native placement fields and both package references across scene JSON", () => {
  const unit = createPlacedDecal(id, source, placement, 8, [3, 4, 5]);
  const document = { $schema: "openscp.lot-asset/1", units: [schemaUnitJson(unit)] };
  writeDecalSources(document, [unit]);
  const saved = JSON.parse(JSON.stringify(document));
  expect(saved.units[0]).toMatchObject({
    id: "decal:1:8",
    decalId: id,
    decalSource: source,
    transform: { matrix: [...placement.basis, 3, 4, 5] },
    category: 1,
    scale: 1.25,
    depth: 2,
    materialData: [0.5, 0, 0],
  });
  const restored = unitsFromSchema(saved);
  restoreDecalSources(saved, restored);
  expect(restored[0]).toEqual(unit);
});

it("restores one texture per source and rebinds it to each decal row", async () => {
  const unit = createPlacedDecal(id, source, placement, 8, [3, 4, 5]);
  const copy: DecalUnit = { ...unit, index: 10 };
  const texture = { category: 0, index: 0, idInstance: id.instance, png: "png" } as DecalUnitTexture;
  vi.mocked(command).mockClear();
  vi.mocked(command).mockResolvedValueOnce({ source, texture, placement: null });
  const decals = useWorkbenchDecals();
  await decals.restore([unit, copy]);
  expect(command).toHaveBeenCalledExactlyOnceWith("resolve_workbench_decal", {
    request: { id, source, placement: false },
  });
  expect(decals.texturesFor([unit, copy], [])).toEqual([
    { ...texture, category: 1, index: 8 },
    { ...texture, category: 1, index: 10 },
  ]);
});
