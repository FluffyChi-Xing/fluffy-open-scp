import { computed, ref, watch, type Ref } from "vue";
import type { Connection } from "@vue-flow/core";
import {
  connectionError,
  defaultMapGraph,
  evaluateMapGraph,
  newMapNode,
  type MapGraph,
  type MapNodeKind,
  type MapGraphNode,
} from "@/lib/map-graph";
export function useMapGraph(project: Ref<string>) {
  const graph = ref<MapGraph>(defaultMapGraph()),
    error = ref(""),
    visible = ref(false);
  watch(
    project,
    () => {
      graph.value = defaultMapGraph();
      error.value = "";
      visible.value = false;
      try {
        const raw = localStorage.getItem("openscp-map-graph:" + project.value);
        if (raw) {
          const saved = JSON.parse(raw);
          if (
            saved.version === 1 &&
            Array.isArray(saved.nodes) &&
            Array.isArray(saved.edges)
          )
            graph.value = saved;
        }
      } catch {
        error.value = "restoreError";
      }
    },
    { immediate: true },
  );
  let executionKey = "",
    previous: ReturnType<typeof evaluateMapGraph> | undefined;
  const result = computed(() => {
    const key = JSON.stringify({
      nodes: graph.value.nodes.map(({ id, kind, config, pixels, strokes }) => ({
        id,
        kind,
        config,
        pixels,
        strokes,
      })),
      edges: graph.value.edges,
    });
    if (key !== executionKey || !previous) {
      executionKey = key;
      previous = evaluateMapGraph(graph.value);
    }
    return previous;
  });
  const nodes = computed(() =>
    visible.value
      ? graph.value.nodes.map((n) => ({
          id: n.id,
          type: "map-generation",
          position: n.position,
          data: {
            node: n,
            field: result.value.fields.get(n.id),
            error: result.value.errors.get(n.id),
          },
        }))
      : [],
  );
  const edges = computed(() => (visible.value ? graph.value.edges : []));
  function save() {
    try {
      localStorage.setItem(
        "openscp-map-graph:" + project.value,
        JSON.stringify(graph.value),
      );
      error.value = "";
    } catch {
      error.value = "saveError";
    }
  }
  function update(node: MapGraphNode) {
    graph.value.nodes = graph.value.nodes.map((n) =>
      n.id === node.id ? node : n,
    );
    save();
  }
  function add(kind: MapNodeKind) {
    const node = newMapNode(
      kind,
      "mapgen-" + crypto.randomUUID(),
      80 + graph.value.nodes.length * 60,
      -600,
    );
    graph.value.nodes.push(node);
    visible.value = true;
    save();
    return node.id;
  }
  function connect(connection: Connection) {
    const edge = {
      id: "mapgen-edge-" + crypto.randomUUID(),
      source: connection.source ?? "",
      target: connection.target ?? "",
      sourceHandle: connection.sourceHandle ?? "",
      targetHandle: connection.targetHandle ?? "",
    };
    const failure = connectionError(graph.value, edge);
    if (failure) {
      error.value = failure;
      return;
    }
    graph.value.edges = [
      ...graph.value.edges.filter(
        (e) =>
          !(e.target === edge.target && e.targetHandle === edge.targetHandle),
      ),
      edge,
    ];
    save();
  }
  function remove(id: string) {
    graph.value.nodes = graph.value.nodes.filter((n) => n.id !== id);
    graph.value.edges = graph.value.edges.filter(
      (e) => e.source !== id && e.target !== id && e.id !== id,
    );
    save();
  }
  function move(id: string, position: { x: number; y: number }) {
    const node = graph.value.nodes.find((n) => n.id === id);
    if (node) update({ ...node, position });
  }
  return {
    graph,
    nodes,
    edges,
    visible,
    error,
    update,
    add,
    connect,
    remove,
    move,
  };
}
