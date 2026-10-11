import { generateHeightField } from "./map-generation";
export type MapNodeKind =
  | "extent"
  | "perlin"
  | "value"
  | "image"
  | "blend"
  | "grass"
  | "trees"
  | "output";
export interface MapGraphNode {
  id: string;
  kind: MapNodeKind;
  position: { x: number; y: number };
  config: Record<string, number>;
  pixels?: number[];
  strokes?: { x: number; y: number; radius: number; strength: number }[];
}
export interface MapGraphEdge {
  id: string;
  source: string;
  target: string;
  sourceHandle: string;
  targetHandle: string;
}
export interface MapGraph {
  version: 1;
  nodes: MapGraphNode[];
  edges: MapGraphEdge[];
}
export interface TerrainField {
  size: number;
  extent: number;
  elevation: number;
  water: number;
  heights: Float32Array;
  grass: Uint8Array;
  trees: Uint8Array;
}
export const inputs: Record<MapNodeKind, string[]> = {
  extent: [],
  perlin: ["extent"],
  value: ["extent"],
  image: ["extent"],
  blend: ["a", "b"],
  grass: ["terrain"],
  trees: ["terrain"],
  output: ["terrain"],
};
export function newMapNode(
  kind: MapNodeKind,
  id: string,
  x: number,
  y: number,
): MapGraphNode {
  return {
    id,
    kind,
    position: { x, y },
    config:
      kind === "extent"
        ? { extent: 8192, elevation: 800, water: 100 }
        : kind === "perlin" || kind === "value"
          ? { seed: 42, frequency: 5, octaves: 5, persistence: 0.5 }
          : kind === "blend"
            ? { mix: 0.5 }
            : kind === "grass" || kind === "trees"
              ? { radius: 8, strength: 0.7 }
              : {},
  };
}
export function defaultMapGraph(): MapGraph {
  const kinds: MapNodeKind[] = ["extent", "perlin", "grass", "trees", "output"];
  const nodes = kinds.map((kind, i) =>
    newMapNode(kind, "mapgen-" + i, i * 370, -1100),
  );
  return {
    version: 1,
    nodes,
    edges: nodes
      .slice(1)
      .map((node, i) => ({
        id: "mapgen-edge-" + i,
        source: nodes[i]!.id,
        target: node.id,
        sourceHandle: i === 0 ? "extent" : "terrain",
        targetHandle: i === 0 ? "extent" : "terrain",
      })),
  };
}
export function connectionError(
  graph: MapGraph,
  edge: MapGraphEdge,
): string | null {
  const source = graph.nodes.find((n) => n.id === edge.source),
    target = graph.nodes.find((n) => n.id === edge.target);
  if (
    !source ||
    !target ||
    source.kind === "output" ||
    !inputs[target.kind].includes(edge.targetHandle)
  )
    return "ports";
  const type = source.kind === "extent" ? "extent" : "terrain";
  if (
    edge.sourceHandle !== type ||
    (edge.targetHandle === "extent" ? "extent" : "terrain") !== type
  )
    return "ports";
  const visit = (id: string, seen = new Set<string>()): boolean => {
    if (id === source.id) return true;
    if (seen.has(id)) return false;
    seen.add(id);
    return graph.edges
      .filter((e) => e.source === id)
      .some((e) => visit(e.target, seen));
  };
  return visit(target.id) ? "cycle" : null;
}
export function evaluateMapGraph(graph: MapGraph) {
  const fields = new Map<string, TerrainField>(),
    errors = new Map<string, string>(),
    visiting = new Set<string>();
  const finite = (
    value: number | undefined,
    fallback: number,
    min: number,
    max: number,
  ) =>
    Number.isFinite(value) ? Math.max(min, Math.min(max, value!)) : fallback;
  function evaluate(id: string): TerrainField | undefined {
    if (fields.has(id)) return fields.get(id);
    if (errors.has(id)) return undefined;
    const node = graph.nodes.find((n) => n.id === id);
    if (!node || node.kind === "extent") return undefined;
    if (visiting.has(id)) {
      errors.set(id, "cycle");
      return;
    }
    visiting.add(id);
    const upstream = new Map<string, MapGraphNode>();
    for (const port of inputs[node.kind]) {
      const edge = graph.edges.find(
        (e) => e.target === id && e.targetHandle === port,
      );
      const source = graph.nodes.find((n) => n.id === edge?.source);
      if (!edge || !source) {
        errors.set(id, "missing");
        visiting.delete(id);
        return;
      }
      if (
        connectionError(
          { ...graph, edges: graph.edges.filter((e) => e.id !== edge.id) },
          edge,
        )
      ) {
        errors.set(id, "ports");
        visiting.delete(id);
        return;
      }
      upstream.set(port, source);
    }
    let field: TerrainField | undefined;
    if (["perlin", "value", "image"].includes(node.kind)) {
      const config = upstream.get("extent")!.config,
        size = 128;
      if (node.kind === "image" && node.pixels?.length !== size * size)
        errors.set(id, "imageMissing");
      else
        field = {
          size,
          extent: finite(config.extent, 8192, 2048, 32768),
          elevation: finite(config.elevation, 800, 1, 10000),
          water: finite(config.water, 100, -1000, 10000),
          heights:
            node.kind === "image"
              ? Float32Array.from(node.pixels!)
              : generateHeightField({
                  width: size,
                  height: size,
                  algorithm: node.kind === "value" ? "value" : "perlin",
                  seed: finite(node.config.seed, 42, 0, 2147483647),
                  octaves: finite(node.config.octaves, 5, 1, 12),
                  frequency: finite(node.config.frequency, 5, 0.01, 256),
                  persistence: finite(node.config.persistence, 0.5, 0, 1),
                  grassBrush: 0,
                  treeBrush: 0,
                }),
          grass: new Uint8Array(size * size),
          trees: new Uint8Array(size * size),
        };
    } else {
      const a = evaluate(
        upstream.get(node.kind === "blend" ? "a" : "terrain")!.id,
      );
      if (!a) errors.set(id, "upstream");
      else {
        field = {
          ...a,
          heights: a.heights.slice(),
          grass: a.grass.slice(),
          trees: a.trees.slice(),
        };
        if (node.kind === "blend") {
          const b = evaluate(upstream.get("b")!.id);
          if (!b) {
            errors.set(id, "upstream");
            field = undefined;
          } else if (
            b.extent !== a.extent ||
            b.elevation !== a.elevation ||
            b.water !== a.water
          ) {
            errors.set(id, "extentMismatch");
            field = undefined;
          } else {
            const mix = finite(node.config.mix, 0.5, 0, 1);
            for (let i = 0; i < a.heights.length; i++)
              field.heights[i] =
                a.heights[i]! * (1 - mix) + b.heights[i]! * mix;
          }
        } else if (node.kind === "grass" || node.kind === "trees") {
          const mask = field[node.kind];
          for (const stroke of node.strokes ?? []) {
            for (
              let y = Math.max(0, Math.floor(stroke.y - stroke.radius));
              y < Math.min(a.size, stroke.y + stroke.radius + 1);
              y++
            )
              for (
                let x = Math.max(0, Math.floor(stroke.x - stroke.radius));
                x < Math.min(a.size, stroke.x + stroke.radius + 1);
                x++
              )
                if (Math.hypot(x - stroke.x, y - stroke.y) <= stroke.radius)
                  mask[y * a.size + x] = Math.round(stroke.strength * 255);
          }
        }
      }
    }
    visiting.delete(id);
    if (field) fields.set(id, field);
    return field;
  }
  graph.nodes.forEach((n) => evaluate(n.id));
  return { fields, errors };
}
