import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import type { ReplicaData } from "@/lib/game-ui/replica/types";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";

/** 复刻数据 fixture：结构同 public/game-ui/replica/data/*.js 的全局。 */
function replicaFixture(): ReplicaData {
  return {
    layout: { instanceID: 1, width: 1600, height: 900 },
    assets: {},
    locale: {},
    model: {
      categories: [
        { id: "CAT_ROAD", label: "道路", icon: null, iconHash: null },
        { id: "CAT_PARK", label: "公園", icon: null, iconHash: null },
      ],
      row: { pitch: 46.85, size: 43, centerX: 875.5, offsetY: 0 },
    },
    tools: {
      CAT_ROAD: [
        { instance: "BBBB0001", label: "低密度泥路", preview: null, marquee: null, locked: false },
        { instance: "BBBB0002", label: "砂石路橋梁", preview: null, marquee: null, locked: false },
        { instance: "BBBB0003", label: "", preview: null, marquee: null, locked: true },
      ],
      CAT_PARK: [],
    },
    palette: {
      byCategory: {},
      row: { pitch: 121.6, startX: 245, slotW: 121.6, slotH: 50, topInBar: 4 },
    },
  };
}

describe("uiWorkbench store（复刻数据模型）", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  function makeStore() {
    const store = useUiWorkbenchStore();
    store.hydrate(replicaFixture());
    return store;
  }

  it("hydrate 后一级菜单为复刻数据的分类列表，默认不选中任何菜单", () => {
    const store = makeStore();
    expect(store.replica).not.toBeNull();
    expect(store.selectedMenuId).toBeNull();
    expect(store.activeMenu).toBeNull();
    expect(store.categories.map((category) => category.id)).toEqual(["CAT_ROAD", "CAT_PARK"]);
    store.enterMenu("CAT_ROAD");
    expect(store.activeMenu?.label).toBe("道路");
    expect(store.activeMenu?.entries.map((entry) => entry.tool.id)).toEqual([
      "BBBB0001",
      "BBBB0002",
      "BBBB0003",
    ]);
  });

  it("工具视图映射：顺序即排序键，空名回退 instance 且标记未解析", () => {
    const store = makeStore();
    store.enterMenu("CAT_ROAD");
    const entries = store.activeMenu?.entries ?? [];
    expect(entries.map((entry) => entry.tool.pos)).toEqual([10, 20, 30]);
    expect(entries[2].tool.label).toBe("BBBB0003");
    expect(entries[2].tool.source).toBe("unresolved");
    expect(entries[0].tool.source).toBe("locale");
  });

  it("enterMenu 钻入二级、leaveMenu 返回一级", () => {
    const store = makeStore();
    store.enterMenu("CAT_ROAD");
    expect(store.entered).toBe(true);
    expect(store.selectedMenuId).toBe("CAT_ROAD");
    store.leaveMenu();
    expect(store.entered).toBe(false);
    expect(store.selectedMenuId).toBe("CAT_ROAD"); // 保持选中，便于面板回看
  });

  it("updateItem 只做覆盖，未编辑字段回落基础数据", () => {
    const store = makeStore();
    store.enterMenu("CAT_ROAD");
    store.updateItem("CAT_ROAD", "BBBB0001", { label: "我的道路" });
    const edited = store.activeMenu?.entries[0].tool;
    expect(edited?.label).toBe("我的道路");
    expect(edited?.pos).toBe(10);
  });

  it("addItem 追加到菜单末位；removeItem 可删除新增项", () => {
    const store = makeStore();
    store.enterMenu("CAT_ROAD");
    store.addItem("CAT_ROAD", "新道路", null);
    const entries = store.activeMenu?.entries ?? [];
    const addedEntry = entries.at(-1);
    expect(addedEntry?.tool.label).toBe("新道路");
    expect(addedEntry?.tool.pos).toBe(40);
    expect(addedEntry?.isNew).toBe(true);

    store.removeItem("CAT_ROAD", addedEntry?.tool.id ?? "");
    expect(
      store.activeMenu?.entries.some((entry) => entry.tool.label === "新道路"),
    ).toBe(false);
  });

  it("addCategory 新建分类并选中；removeCategory 可删除并回到未选中", () => {
    const store = makeStore();
    store.addCategory("新分类");
    expect(store.categories.at(-1)?.id).toBe(store.selectedMenuId);
    expect(store.activeMenu?.label).toBe("新分类");
    store.removeCategory(store.selectedMenuId ?? "");
    expect(store.categories.some((c) => c.label === "新分类")).toBe(false);
    expect(store.selectedMenuId).toBeNull();
    expect(store.activeMenu).toBeNull();
  });

  it("exportOverlay 携带编辑/新增/自定义分类；resetAll 清空", () => {
    const store = makeStore();
    store.updateItem("CAT_ROAD", "BBBB0001", { label: "我的道路" });
    store.addItem("CAT_ROAD", "新道路", null);
    store.addCategory("新分类");
    const overlay = JSON.parse(store.exportOverlay());
    expect(overlay.format).toBe("openscp-ui-workbench-overlay/1");
    expect(overlay.edits["BBBB0001"].label).toBe("我的道路");
    expect(overlay.added.CAT_ROAD).toHaveLength(1);
    expect(overlay.customCategories).toHaveLength(1);

    store.resetAll();
    expect(Object.keys(store.edits)).toHaveLength(0);
    expect(store.activeMenu?.entries.some((entry) => entry.isNew)).toBe(false);
  });
});
