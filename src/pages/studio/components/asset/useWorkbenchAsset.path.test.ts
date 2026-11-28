import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import { unitsFromSchema } from "@/pages/packages/components/property-editor/peSchemaDoc";
import { useWorkbenchAsset } from "./useWorkbenchAsset";

it("finishes a path and reserves its undone point ID when drawing the next path", async () => {
  const read = vi.spyOn(tauriApi.workspace, "codeReadText").mockResolvedValue({
    relativePath: "asset.json",
    revision: "test",
    size: 0,
    content: JSON.stringify({
      $schema: "openscp.lot-asset/1",
      units: [],
      editor: { workbench: { source: "none", modelRemoved: true } },
    }),
  });
  const tree = vi.spyOn(tauriApi.workspace, "codeTree").mockResolvedValue({
    rootPath: "D:/path-test", fileCount: 0, folderCount: 0, truncated: false, entries: [],
  });
  let asset!: ReturnType<typeof useWorkbenchAsset>;
  const wrapper = mount(defineComponent({
    setup() {
      asset = useWorkbenchAsset("path-test");
      return () => h("div");
    },
  }));
  try {
    expect(await asset.restoreFromProject()).toBe(true);
    const first = asset.createPath();
    expect(asset.addPathPoint([0, 0, 0])).toBe("pathPoint:0");
    expect(asset.addPathPoint([10, 0, 0])).toBe("pathPoint:1");
    expect(asset.addPathPoint([20, 0, 0])).toBe("pathPoint:2");
    asset.ui.value!.edit.undo();
    asset.finishPath();
    expect(asset.activePathId.value).toBeNull();
    expect(asset.addPathPoint([10, 30, 0])).toBeNull();

    const second = asset.createPath();
    expect(second).not.toBe(first);
    expect(asset.addPathPoint([0, 30, 0])).toBe("pathPoint:3");
    expect(asset.addPathPoint([20, 30, 0])).toBe("pathPoint:4");
    asset.finishPath();

    const saved = JSON.parse(asset.schemaJson.value);
    expect(saved.editor.workbench.paths.map((path: { pointIds: string[] }) => path.pointIds))
      .toEqual([
        ["pathPoint:0", "pathPoint:1"],
        ["pathPoint:3", "pathPoint:4"],
      ]);
    expect(unitsFromSchema(saved).filter((unit) => unit.kind === "pathPoint").map((unit) => [unit.index, unit.point]))
      .toEqual([[0, [0, 0, 0]], [1, [10, 0, 0]], [3, [0, 30, 0]], [4, [20, 30, 0]]]);
  } finally {
    wrapper.unmount();
    read.mockRestore();
    tree.mockRestore();
  }
});
