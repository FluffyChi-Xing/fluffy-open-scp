import { describe, expect, it } from "vitest";
import { reactive } from "vue";
import {
  LOGIC_NODE_SPECS,
  addLogicNode,
  cloneLogicParams,
  duplicateLogicNode,
  connectLogic,
  defaultEducationParams,
  emptyLogicGraph,
  ensureCoreNode,
  leafSpec,
  leafTypeOf,
  logicIssues,
  normalizeLogicGraph,
  removeLogicNode,
  updateLogicParams,
  type BehaviorCoreParams,
  type LogicNodeSpec,
} from "./logicGraph";

const spec = (kind: LogicNodeSpec["kind"], leafType?: LogicNodeSpec["leafType"]) =>
  LOGIC_NODE_SPECS.find(
    (item) => item.kind === kind && item.leafType === leafType,
  )!;

const infoSpec = spec("building-info");
const coreSpec = spec("behavior-core");
const educationSpec = leafSpec("education");

describe("addLogicNode / ensureCoreNode", () => {
  it("creates stable ids and default params", () => {
    const first = addLogicNode(emptyLogicGraph(), infoSpec, [10, 20]);
    expect(first.node.id).toBe("info-1");
    expect(first.graph.params["info-1"]).toEqual({
      w1: 20,
      w2: 5,
      w3: 0,
      minWorkers: 5,
      schedule: [6, 6, 7, 7, 6, 6, 7, 6],
    });
    const second = addLogicNode(first.graph, infoSpec, [30, 40]);
    expect(second.node.id).toBe("info-2");
  });

  it("ensureCoreNode reuses an existing aggregate", () => {
    const withCore = addLogicNode(emptyLogicGraph(), coreSpec, [0, 0]);
    const again = ensureCoreNode(withCore.graph, [50, 50]);
    expect(again.node.id).toBe("core-1");
    expect(again.graph.nodes).toHaveLength(1);
  });
});

describe("connectLogic", () => {
  it("connects info/core to the asset ports and leaf to core", () => {
    let graph = emptyLogicGraph();
    graph = addLogicNode(graph, infoSpec, [0, 0]).graph;
    graph = addLogicNode(graph, coreSpec, [0, 100]).graph;
    graph = addLogicNode(graph, educationSpec, [0, 200]).graph;

    const info = connectLogic(graph, "info-1", "asset", "info");
    expect(info.error).toBeNull();
    graph = info.graph;
    const core = connectLogic(graph, "core-1", "asset", "behavior");
    expect(core.error).toBeNull();
    graph = core.graph;
    const leaf = connectLogic(graph, "leaf-education-1", "core-1", "leaf");
    expect(leaf.error).toBeNull();
    graph = leaf.graph;

    expect(graph.edges).toHaveLength(3);
    expect(logicIssues(graph)).toEqual([]);
  });

  it("rejects leaf to asset and info to the wrong port", () => {
    let graph = addLogicNode(emptyLogicGraph(), educationSpec, [0, 0]).graph;
    expect(connectLogic(graph, "leaf-education-1", "asset", "info").error).toBe(
      "leafRequiresCore",
    );
    graph = addLogicNode(graph, infoSpec, [0, 40]).graph;
    expect(connectLogic(graph, "info-1", "asset", "behavior").error).toBe(
      "nodeRequiresAsset",
    );
  });

  it("keeps one anchor edge per info/core node", () => {
    let graph = addLogicNode(emptyLogicGraph(), infoSpec, [0, 0]).graph;
    graph = connectLogic(graph, "info-1", "asset", "info").graph;
    graph = connectLogic(graph, "info-1", "asset", "info").graph;
    expect(graph.edges).toHaveLength(1);
  });
});

describe("removeLogicNode / duplicateLogicNode", () => {
  it("refuses to delete a core that still has leaves", () => {
    let graph = addLogicNode(emptyLogicGraph(), coreSpec, [0, 0]).graph;
    graph = addLogicNode(graph, educationSpec, [0, 100]).graph;
    graph = connectLogic(graph, "core-1", "asset", "behavior").graph;
    graph = connectLogic(graph, "leaf-education-1", "core-1", "leaf").graph;
    const result = removeLogicNode(graph, "core-1");
    expect(result.blocked).toBe("coreHasLeaves");
    expect(result.graph.nodes).toHaveLength(2);
  });

  it("removing a leaf drops its edges and params", () => {
    let graph = addLogicNode(emptyLogicGraph(), coreSpec, [0, 0]).graph;
    graph = addLogicNode(graph, educationSpec, [0, 100]).graph;
    graph = connectLogic(graph, "leaf-education-1", "core-1", "leaf").graph;
    const result = removeLogicNode(graph, "leaf-education-1");
    expect(result.blocked).toBeNull();
    expect(result.graph.edges).toEqual([]);
    expect(result.graph.params["leaf-education-1"]).toBeUndefined();
  });
});

describe("logicIssues", () => {
  it("flags unattached nodes and missing aggregate", () => {
    const graph = addLogicNode(emptyLogicGraph(), educationSpec, [0, 0]).graph;
    const codes = logicIssues(graph).map((issue) => issue.code);
    expect(codes).toContain("leafUnattached");
  });

  it("flags duplicate info nodes", () => {
    let graph = addLogicNode(emptyLogicGraph(), infoSpec, [0, 0]).graph;
    graph = addLogicNode(graph, infoSpec, [0, 100]).graph;
    const codes = logicIssues(graph)
      .filter((issue) => issue.code === "duplicateInfo")
      .map((issue) => issue.nodeId);
    expect(codes).toEqual(["info-2"]);
  });

  it("flags explicit utilities without amount", () => {
    let graph = addLogicNode(emptyLogicGraph(), coreSpec, [0, 0]).graph;
    graph = updateLogicParams(graph, "core-1", {
      power: { mode: "explicit", rate: 72, amount: null },
    });
    const codes = logicIssues(graph).map((issue) => issue.code);
    expect(codes).toContain("coreAmount:power");
  });

  it("accepts a complete education leaf", () => {
    let graph = addLogicNode(emptyLogicGraph(), coreSpec, [0, 0]).graph;
    graph = addLogicNode(graph, educationSpec, [0, 100]).graph;
    graph = connectLogic(graph, "core-1", "asset", "behavior").graph;
    graph = connectLogic(graph, "leaf-education-1", "core-1", "leaf").graph;
    const params = graph.params["core-1"] as BehaviorCoreParams;
    expect(params.power.mode).toBe("inherit");
    expect(logicIssues(graph)).toEqual([]);
  });
});

describe("reactive proxies (DataCloneError regression)", () => {
  it("deep-clones params held in a Vue reactive graph", () => {
    const proxy = reactive({ w1: 20, nested: { amount: 90 } });
    expect(cloneLogicParams(proxy)).toEqual({ w1: 20, nested: { amount: 90 } });
    // structuredClone(proxy) throws DataCloneError; the JSON clone must not.
    expect(() => structuredClone(proxy)).toThrow();
  });

  it("duplicates a node from a reactive graph without throwing", () => {
    const base = addLogicNode(emptyLogicGraph(), infoSpec, [0, 0]).graph;
    const graph = reactive(base) as typeof base;
    const result = duplicateLogicNode(graph, "info-1");
    expect(result.node?.id).toBe("info-1-copy-1");
    expect(result.graph.params["info-1-copy-1"]).toEqual({
      w1: 20,
      w2: 5,
      w3: 0,
      minWorkers: 5,
      schedule: [6, 6, 7, 7, 6, 6, 7, 6],
    });
  });
});

describe("normalizeLogicGraph", () => {
  it("migrates legacy documents to an empty graph", () => {
    expect(normalizeLogicGraph(undefined)).toEqual(emptyLogicGraph());
    expect(normalizeLogicGraph({})).toEqual(emptyLogicGraph());
  });

  it("drops dangling edges and fills missing params", () => {
    const graph = normalizeLogicGraph({
      nodes: [
        { id: "core-1", kind: "behavior-core", position: [0, 0] },
        { id: "leaf-education-1", kind: "behavior-leaf", position: [0, 100] },
      ],
      edges: [
        { id: "e1", source: "leaf-education-1", target: "core-1", targetHandle: "leaf" },
        { id: "e2", source: "ghost", target: "asset", targetHandle: "info" },
      ],
      params: {
        "leaf-education-1": { type: "education", capacity: 300 },
      },
    });
    expect(graph.nodes).toHaveLength(2);
    expect(graph.edges.map((edge) => edge.id)).toEqual(["e1"]);
    expect(graph.params["core-1"]).toMatchObject({
      power: { mode: "inherit" },
    });
    expect(graph.params["leaf-education-1"]).toEqual({
      type: "education",
      capacity: 300,
    });
  });

  it("keeps leaf types readable for rendering", () => {
    const graph = normalizeLogicGraph({
      nodes: [
        { id: "leaf-tourism-1", kind: "behavior-leaf", position: [0, 0] },
      ],
      edges: [],
      params: { "leaf-tourism-1": { type: "tourism" } },
    });
    expect(leafTypeOf(graph, graph.nodes[0])).toBe("tourism");
  });
});

describe("defaults", () => {
  it("education defaults match the verified minimal recipe shape", () => {
    expect(defaultEducationParams()).toMatchObject({
      capacity: 250,
      walkPath: true,
      servicePath: true,
      pedestrianPath: false,
    });
  });
});
