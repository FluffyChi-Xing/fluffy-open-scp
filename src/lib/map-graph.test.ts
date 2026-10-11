import { describe, it, expect } from "vitest";
import {
  defaultMapGraph,
  evaluateMapGraph,
  newMapNode,
  connectionError,
  type MapGraphEdge,
} from "./map-graph";
describe("map graph execution", () => {
  it("executes independent source, noise, brush and output nodes", () => {
    const graph = defaultMapGraph();
    graph.nodes[2]!.strokes = [{ x: 64, y: 64, radius: 8, strength: 1 }];
    const result = evaluateMapGraph(graph);
    expect(result.errors.size).toBe(0);
    expect(result.fields.get("mapgen-4")!.grass[64 * 128 + 64]).toBe(255);
    expect(result.fields.get("mapgen-1")!.grass[64 * 128 + 64]).toBe(0);
  });
  it("invalidates downstream outputs after disconnection", () => {
    const graph = defaultMapGraph();
    graph.edges = graph.edges.slice(1);
    const result = evaluateMapGraph(graph);
    expect(result.fields.has("mapgen-4")).toBe(false);
    expect(result.errors.get("mapgen-1")).toBe("missing");
  });
  it("rejects incompatible ports and cycles", () => {
    const graph = defaultMapGraph();
    const edge: MapGraphEdge = {
      id: "edge",
      source: "mapgen-0",
      sourceHandle: "extent",
      target: "mapgen-2",
      targetHandle: "terrain",
    };
    expect(connectionError(graph, edge)).toBe("ports");
    expect(
      connectionError(graph, {
        ...edge,
        source: "mapgen-3",
        sourceHandle: "terrain",
      }),
    ).toBe("cycle");
  });
  it("blends connected grayscale and noise outputs by weight", () => {
    const graph = defaultMapGraph();
    const image = newMapNode("image", "image", 0, 0);
    image.pixels = Array(16384).fill(1);
    const blend = newMapNode("blend", "blend", 0, 0);
    blend.config.mix = 0.25;
    graph.nodes.push(image, blend);
    graph.edges.push(
      {
        id: "e1",
        source: "mapgen-0",
        sourceHandle: "extent",
        target: "image",
        targetHandle: "extent",
      },
      {
        id: "e2",
        source: "mapgen-1",
        sourceHandle: "terrain",
        target: "blend",
        targetHandle: "a",
      },
      {
        id: "e3",
        source: "image",
        sourceHandle: "terrain",
        target: "blend",
        targetHandle: "b",
      },
    );
    const result = evaluateMapGraph(graph);
    expect(result.errors.size).toBe(0);
    expect(result.fields.get("blend")!.heights[42]).toBeCloseTo(
      result.fields.get("mapgen-1")!.heights[42]! * 0.75 + 0.25,
    );
  });
  it("requires actual imported pixels instead of substituting noise", () => {
    const graph = defaultMapGraph();
    graph.nodes[1]!.kind = "image";
    expect(evaluateMapGraph(graph).errors.get("mapgen-1")).toBe("imageMissing");
  });
});
