import {describe, it, expect} from "vitest";
import {effectScope, ref} from "vue";
import {useMapGraph} from "./useMapGraph";
describe("map graph editor", () => {
  it("preserves terrain identity while moving nodes and restores saved connections", () => {
    const scope = effectScope();
    scope.run(() => {
      const project = ref("map-graph-test-" + Math.random());
      const editor = useMapGraph(project);
      editor.visible.value = true;
      const terrain = editor.nodes.value.find(n => n.id === "mapgen-4")!.data.field;
      editor.move("mapgen-1", {x: 400, y: 200});
      expect(editor.nodes.value.find(n => n.id === "mapgen-4")!.data.field).toBe(terrain);
      editor.remove("mapgen-edge-0");
      expect(editor.nodes.value.find(n => n.id === "mapgen-4")!.data.field).toBeUndefined();
      const restored = useMapGraph(project);
      expect(restored.graph.value.edges).toHaveLength(3);
      expect(restored.graph.value.nodes.find(n => n.id === "mapgen-1")!.position).toEqual({x: 400, y: 200});
      localStorage.removeItem("openscp-map-graph:" + project.value);
    });
    scope.stop();
  });
});
