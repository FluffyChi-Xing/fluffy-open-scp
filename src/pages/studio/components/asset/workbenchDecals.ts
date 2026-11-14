import { shallowRef } from "vue";
import { command, type DecalUnit, type DecalUnitTexture, type LotUnitDto, type Tgi, type WorkbenchDecalSource } from "@/api/tauri";
import { unitId } from "@/pages/packages/components/property-editor/unitGizmos";

export interface DecalSelection {
  id: Tgi;
  source: WorkbenchDecalSource;
}

interface DecalPlacement {
  category: number;
  basis: number[];
  scale: number;
  depth: number;
  materialData: [number, number, number];
}

interface ResolvedDecal {
  source: WorkbenchDecalSource;
  texture: DecalUnitTexture;
  placement: DecalPlacement | null;
}

export function createPlacedDecal(
  id: Tgi,
  source: WorkbenchDecalSource,
  placement: DecalPlacement,
  index: number,
  position: [number, number, number],
): DecalUnit {
  return {
    kind: "decal",
    index,
    category: placement.category,
    decalId: id,
    decalSource: source,
    transform: { matrix: [...placement.basis, ...position] },
    scale: placement.scale,
    depth: placement.depth,
    materialData: placement.materialData,
    fields: [],
  };
}

function selection(unit: LotUnitDto): DecalSelection | undefined {
  if (unit.kind === "decal" && unit.decalId && unit.decalSource) {
    return { id: unit.decalId, source: unit.decalSource };
  }
}

function sourceKey({ id, source }: DecalSelection): string {
  return JSON.stringify([
    source.packagePath, source.atlas.typeId, source.atlas.group,
    source.atlas.instance, id.typeId, id.group, id.instance,
  ]);
}

/** Workbench-only fields alongside the existing scene unit contract. */
export function writeDecalSources(document: Record<string, unknown>, units: LotUnitDto[]): void {
  const sources = new Map<string, DecalSelection>();
  for (const unit of units) {
    const decal = selection(unit);
    if (decal) sources.set(unitId(unit), decal);
  }
  for (const row of document.units as Record<string, unknown>[]) {
    const decal = sources.get(String(row.id));
    if (decal) Object.assign(row, { decalId: decal.id, decalSource: decal.source });
  }
}

function tgi(value: unknown): value is Tgi {
  if (!value || typeof value !== "object") return false;
  const key = value as Partial<Tgi>;
  return [key.typeId, key.group, key.instance].every(
    (part) => typeof part === "number" && Number.isInteger(part) && part >= 0 && part <= 0xffff_ffff,
  );
}

export function restoreDecalSources(document: Record<string, unknown>, units: LotUnitDto[]): void {
  const rows = document.units as Record<string, unknown>[];
  for (const [index, unit] of units.entries()) {
    if (unit.kind !== "decal") continue;
    const row = rows[index];
    const source = row.decalSource as Partial<WorkbenchDecalSource> | undefined;
    if (tgi(row.decalId) && source && typeof source.packagePath === "string" && tgi(source.atlas)) {
      unit.decalId = row.decalId;
      unit.decalSource = {
        packagePath: source.packagePath,
        atlas: source.atlas,
        rasterPackagePath: typeof source.rasterPackagePath === "string" ? source.rasterPackagePath : undefined,
      };
    }
  }
}

export function useWorkbenchDecals() {
  const textures = shallowRef(new Map<string, DecalUnitTexture>());
  const placements = new Map<string, ResolvedDecal>();
  let generation = 0;

  function remember(decal: DecalSelection, texture: DecalUnitTexture): void {
    const next = new Map(textures.value);
    next.set(sourceKey(decal), texture);
    textures.value = next;
  }

  async function resolve(decal: DecalSelection, placement: boolean): Promise<ResolvedDecal> {
    const key = sourceKey(decal);
    const cached = placement && placements.get(key);
    if (cached) return cached;
    const current = generation;
    const result = await command<ResolvedDecal>("resolve_workbench_decal", {
      request: { ...decal, placement },
    });
    if (current === generation) {
      remember(decal, result.texture);
      if (placement) placements.set(key, result);
    }
    return result;
  }

  async function restore(units: LotUnitDto[]): Promise<void> {
    const missing = new Map<string, DecalSelection>();
    for (const unit of units) {
      const decal = selection(unit);
      if (decal && !textures.value.has(sourceKey(decal))) missing.set(sourceKey(decal), decal);
    }
    await Promise.allSettled([...missing.values()].map((decal) => resolve(decal, false)));
  }

  function texturesFor(units: LotUnitDto[], original: DecalUnitTexture[]): DecalUnitTexture[] {
    const authored = units.filter((unit): unit is DecalUnit => unit.kind === "decal" && !!selection(unit));
    if (!authored.length) return original;
    const replaced = new Set(authored.map((unit) => unitId(unit)));
    const result = original.filter((texture) => !replaced.has(`decal:${texture.category}:${texture.index}`));
    for (const unit of authored) {
      const texture = textures.value.get(sourceKey(selection(unit)!));
      if (texture) result.push({ ...texture, category: unit.category, index: unit.index });
    }
    return result;
  }

  function reset(): void {
    generation += 1;
    textures.value = new Map();
    placements.clear();
  }

  return { resolve, restore, texturesFor, reset };
}
