import { computed, onBeforeUnmount, onMounted, shallowRef } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { tauriApi } from "@/api";
import type { Region3DData, RegionSummary, StateSource } from "@/lib/region-map";

const schemaPath = "schemas/map_resources/schema.json";
export const windDirectionId = 0x3136055a;
export const layers = [
  { key: "coal", id: 0xd779c976, color: "#bba99a" },
  { key: "ore", id: 0xd74af22b, color: "#e9ba68" },
  { key: "oil", id: 0x76d10ff2, color: "#a39ae3" },
  { key: "cityWaterTable", id: 0x0f92dc56, color: "#69adeb" },
  { key: "wind", id: windDirectionId, color: "#73cdd0" },
  { key: "radiation", id: 0x1d716f2e, color: "#8fc377" },
  { key: "groundPollution", id: 0xf69df1c6, color: "#cc92ca" },
] as const;
interface Source { packagePath: string; group: string; saveDirectory?: string }
interface Stroke {
  site: string;
  layer: number;
  center: [number, number];
  radius: number;
  value: number;
  gesture?: number;
  profile?: "flat" | "feathered" | "direction";
}
interface Schema {
  $schema: "openscp.map-resources"; version: 1; source: Source; fingerprint: string; strokes: Stroke[];
}
type City = Pick<StateSource, "site" | "bounds" | "maps"> & { environment: string; windDirection?: number | null };

function applyStrokes(cities: City[], strokes: Stroke[]): City[] {
  const bySite = new Map<string, Stroke[]>();
  for (const stroke of strokes) {
    if (stroke.profile === "direction") continue;
    const entries = bySite.get(stroke.site) ?? [];
    entries.push(stroke);
    bySite.set(stroke.site, entries);
  }
  return cities.map((city) => {
    const cityStrokes = bySite.get(city.site);
    if (!city.bounds || !cityStrokes?.length) return city;
    const [x0, y0, x1, y1] = city.bounds;
    return { ...city, maps: city.maps.map((map) => {
      const mapStrokes = cityStrokes.filter((stroke) => stroke.layer === map.id);
      if (!mapStrokes.length) return map;
      const values = map.values.slice();
      const dx = (x1 - x0) / map.size, dy = (y1 - y0) / map.size;
      const clamp = (n: number) => Math.max(0, Math.min(map.size - 1, Math.floor(n)));
      for (const { center: [x, y], radius, value, profile } of mapStrokes) {
        const top = clamp((y - radius - y0) / dy), bottom = clamp((y + radius - y0) / dy);
        const left = clamp((x - radius - x0) / dx), right = clamp((x + radius - x0) / dx);
        const radiusSquared = radius * radius;
        for (let row = top; row <= bottom; row++) {
          const offsetY = y0 + (row + 0.5) * dy - y;
          for (let col = left; col <= right; col++) {
            const offsetX = x0 + (col + 0.5) * dx - x;
            const distanceSquared = offsetX * offsetX + offsetY * offsetY;
            if (distanceSquared > radiusSquared) continue;
            const index = row * map.size + col;
            if (profile === "feathered") {
              const weight = Math.max(0, 1 - distanceSquared / radiusSquared);
              const amount = Math.round(value * weight * weight);
              if (amount > 0) values[index] = Math.max(values[index]!, amount);
            } else {
              values[index] = value;
            }
          }
        }
      }
      return { ...map, values };
    }) };
  });
}

function makeBrushPreview() {
  let source: City[] | undefined;
  let recent: { strokes: Stroke[]; cities: City[] }[] = [];
  return (baseline: City[], strokes: Stroke[]) => {
    if (source !== baseline) {
      source = baseline;
      recent = [];
    }
    // Immutable gestures retain the stroke references of their prefix. Keep a
    // few recent snapshots so a normal stroke or undo need not replay history.
    const base = recent.find(({ strokes: previous }) =>
      previous.length <= strokes.length && previous.every((stroke, i) => stroke === strokes[i]));
    if (base?.strokes.length === strokes.length) return base.cities;
    const cities = applyStrokes(base?.cities ?? baseline, strokes.slice(base?.strokes.length ?? 0));
    recent = [{ strokes, cities }, ...recent].slice(0, 8);
    return cities;
  };
}

export function useMapResources(project: string) {
  const packagePath = shallowRef("");
  const saveDirectory = shallowRef("");
  const group = shallowRef("");
  const regions = shallowRef<RegionSummary[]>([]);
  const data = shallowRef<Region3DData | null>(null);
  const baseline = shallowRef<City[]>([]);
  const schema = shallowRef<Schema | null>(null);
  const revision = shallowRef<string | null>(null);
  const selected = shallowRef<string | null>(null);
  const busy = shallowRef(true);
  const error = shallowRef("");
  const dirty = shallowRef(false);
  const choosing = shallowRef(false);
  let sequence = 0;
  const brushPreview = makeBrushPreview();
  const cities = computed(() => brushPreview(baseline.value, schema.value?.strokes ?? []));
  const activeCity = computed(() => cities.value.find((city) => city.site === selected.value));
  const windArrows = computed(() => {
    const edits = new Map<string, Stroke>();
    for (const stroke of schema.value?.strokes ?? []) {
      if (stroke.profile === "direction") edits.set(stroke.site, stroke);
    }
    return baseline.value.flatMap(({ site, bounds, windDirection }) => {
      if (!bounds || windDirection == null) return [];
      const edit = edits.get(site);
      // Keep stock arrows clear of the slot name at the center of the city.
      const [x, y] = edit?.center ?? [(bounds[0] + bounds[2]) / 2, bounds[1] + (bounds[3] - bounds[1]) * 0.3];
      return [{ site, x, y, direction: edit?.value ?? windDirection }];
    });
  });
  const renderResources = computed(() => data.value ? {
    ...data.value,
    windArrows: windArrows.value,
    staticLayers: {
      sources: cities.value.map((city) => ({ ...city, origin: [0, 0] as [number, number], curves: [] })),
    },
  } : undefined);
  const abundance = computed(() => new Map(cities.value.map((city, index) => {
    const base = baseline.value[index];
    return [city.site, layers.filter((layer) => layer.key !== "wind"
      && (layer.key !== "groundPollution" || schema.value?.source.saveDirectory)).map((layer) => {
      const map = city.maps.find((candidate) => candidate.id === layer.id);
      const original = base.maps.find((candidate) => candidate.id === layer.id);
      return {
        ...layer,
        coverage: map ? map.values.filter((value) => value > 0).length / map.values.length * 100 : null,
        changed: !!map && !!original && map.values.some((value, i) => value !== original.values[i]),
      };
    })];
  })));

  function flow<T>(action: string, payload: object): Promise<T> {
    return invoke("code_flow", { request: { project, action, payload } });
  }
  function report(key: string, cause: unknown) {
    console.error(cause);
    error.value = key;
  }
  async function list(path: string) {
    const token = ++sequence;
    packagePath.value = path;
    group.value = "";
    regions.value = [];
    if (!path) { busy.value = false; return; }
    busy.value = true;
    error.value = "";
    try {
      const result = await invoke<RegionSummary[]>("map_panel_list_regions", { packagePath: path });
      if (token !== sequence) return;
      regions.value = result;
      if (!result.length) error.value = "loadFailed";
      if (result.length === 1) group.value = result[0]?.group ?? "";
    } catch (cause) {
      if (token === sequence) report("loadFailed", cause);
    } finally {
      if (token === sequence) busy.value = false;
    }
  }
  async function chooseFile() {
    const path = await open({ multiple: false, filters: [{ name: "DBPF", extensions: ["package"] }] });
    if (typeof path === "string") await list(path);
  }
  async function chooseSave() {
    const path = await open({ multiple: false, directory: true });
    if (typeof path === "string") saveDirectory.value = path;
  }
  async function load(source: Source, saved?: Schema) {
    const token = ++sequence;
    busy.value = true;
    error.value = "";
    try {
      const [region, payload] = await Promise.all([
        invoke<Region3DData>("map_panel_region_3d", { packagePath: source.packagePath, group: source.group }),
        flow<{ cities: City[]; fingerprint: string }>("map-resource-preview", source),
      ]);
      if (token !== sequence) return;
      if (saved && saved.fingerprint !== payload.fingerprint) {
        error.value = "sourceChanged";
        return;
      }
      baseline.value = payload.cities;
      packagePath.value = source.packagePath;
      saveDirectory.value = source.saveDirectory ?? "";
      group.value = source.group;
      schema.value = saved ?? { $schema: "openscp.map-resources", version: 1, source, fingerprint: payload.fingerprint, strokes: [] };
      data.value = region;
      selected.value = null;
      choosing.value = false;
      dirty.value = !saved;
    } catch (cause) {
      if (token === sequence) report("loadFailed", cause);
    } finally {
      if (token === sequence) busy.value = false;
    }
  }
  function openRegion() {
    if (packagePath.value && group.value) return load({
      packagePath: packagePath.value, group: group.value,
      saveDirectory: saveDirectory.value || undefined,
    });
  }
  function stamp(layer: number, points: [number, number][], radius: number, value: number) {
    const city = activeCity.value;
    const current = schema.value;
    if (!current || !city?.bounds || busy.value || current.strokes.length >= 4096) return;
    const [x0, y0, x1, y1] = city.bounds;
    const gesture = current.strokes.length;
    const profile = value === 0 ? "flat" : "feathered";
    const additions = points.filter(([x, y]) => x >= x0 && x < x1 && y >= y0 && y < y1)
      .slice(0, 4096 - gesture).map<Stroke>((center) => ({ site: city.site, layer, center, radius, value, gesture, profile }));
    if (!additions.length) return;
    schema.value = { ...current, strokes: [...current.strokes, ...additions] };
    dirty.value = true;
  }
  function placeWind(point: [number, number], direction: number) {
    const city = activeCity.value;
    const current = schema.value;
    if (!current || !city?.bounds || city.windDirection == null || busy.value || current.strokes.length >= 4096) return;
    const [x0, y0, x1, y1] = city.bounds;
    const [x, y] = point;
    if (!(x >= x0 && x < x1 && y >= y0 && y < y1)
      || !Number.isInteger(direction) || direction < 0 || direction >= 360) return;
    const arrow: Stroke = {
      site: city.site, layer: windDirectionId, center: point, value: direction,
      radius: 0, profile: "direction", gesture: current.strokes.length,
    };
    schema.value = { ...current, strokes: [...current.strokes, arrow] };
    dirty.value = true;
  }
  function undo() {
    if (!schema.value?.strokes.length) return;
    const end = schema.value.strokes.at(-1)?.gesture;
    schema.value = { ...schema.value, strokes: schema.value.strokes.slice(0, end ?? -1) };
    dirty.value = true;
  }
  async function save() {
    if (!schema.value || busy.value) return false;
    if (!dirty.value) return true;
    busy.value = true;
    error.value = "";
    try {
      const output = await flow<{ revision: string }>("map-resource-save", { schema: schema.value, revision: revision.value });
      revision.value = output.revision;
      dirty.value = false;
      return true;
    } catch (cause) {
      report("saveFailed", cause);
      return false;
    } finally {
      busy.value = false;
    }
  }
  onMounted(async () => {
    try {
      const document = await tauriApi.workspace.codeReadText(project, schemaPath);
      revision.value = document.revision;
      const saved = JSON.parse(document.content) as Schema;
      if (saved.$schema !== "openscp.map-resources" || saved.version !== 1
        || typeof saved.source?.packagePath !== "string" || typeof saved.source.group !== "string"
        || !Array.isArray(saved.strokes)
        || saved.strokes.some((stroke) => !Array.isArray(stroke.center) || stroke.center.length !== 2
          || (stroke.profile !== undefined && stroke.profile !== "flat"
            && stroke.profile !== "feathered" && stroke.profile !== "direction"))) {
        throw new Error("Invalid map resource schema");
      }
      await load(saved.source, saved);
    } catch (cause) {
      if ((cause as { code?: string }).code !== "not_found") report("loadFailed", cause);
      busy.value = false;
    }
  });
  onBeforeUnmount(() => { sequence++; });
  return { packagePath, saveDirectory, group, regions, data, schema, selected, busy, error, dirty, choosing,
    activeCity, renderResources, abundance, list, chooseFile, chooseSave, openRegion, stamp, placeWind, undo, save };
}
