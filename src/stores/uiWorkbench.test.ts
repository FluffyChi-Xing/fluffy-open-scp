import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import type { WorkbenchData } from "@/lib/game-ui/workbench";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";

function fixture(): WorkbenchData {
  return {
    meta: { source: "test", toolCount: 4, designResolution: [1024, 793], note: "" },
    assets: { paletteBtn: "/game-ui/png/00000000_2EF08DEB.png" },
    university: {
      label: "大學",
      tools: [
        { id: "AAAA0001", label: "宿舍", pos: 100, icon: null, source: "locale" },
        { id: "AAAA0002", label: "商務學院", pos: 300, icon: null, source: "locale" },
      ],
    },
    city: {
      categories: [
        {
          id: "road",
          label: "道路",
          icon: null,
          items: [
            { id: "BBBB0001", label: "低密度泥路", pos: 10, icon: null, source: "locale" },
            { id: "BBBB0002", label: "砂石路橋梁", pos: 20, icon: null, source: "locale" },
          ],
        },
        { id: "power", label: "电力", icon: null, items: [] },
        { id: "trade", label: "贸易", icon: null, items: [] },
        { id: "landmark", label: "地标", icon: null, items: [] },
        { id: "mayor", label: "市政", icon: null, items: [] },
      ],
    },
  };
}

describe("uiWorkbench store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  function makeStore() {
    const store = useUiWorkbenchStore();
    store.data = fixture();
    store.selectDimension("city");
    return store;
  }

  it("默认 city 维度，一级菜单为该维度的分类列表", () => {
    const store = makeStore();
    expect(store.dimension).toBe("city");
    expect(store.selectedMenuId).toBe("road");
    expect(store.activeMenu?.label).toBe("道路");
    expect(store.activeMenu?.entries.map((entry) => entry.tool.id)).toEqual([
      "BBBB0001",
      "BBBB0002",
    ]);
  });

  it("selectDimension 切换维度：一级内容自动更新、二级状态重置", () => {
    const store = makeStore();
    store.enterMenu("road");
    expect(store.entered).toBe(true);
    store.selectDimension("bigbiz");
    expect(store.dimension).toBe("bigbiz");
    expect(store.entered).toBe(false);
    expect(store.categories.map((category) => category.id)).toEqual([
      "trade",
      "landmark",
      "mayor",
    ]);
    expect(store.selectedMenuId).toBe("trade");
  });

  it("enterMenu 钻入二级、leaveMenu 返回一级", () => {
    const store = makeStore();
    store.enterMenu("road");
    expect(store.entered).toBe(true);
    expect(store.selectedMenuId).toBe("road");
    store.leaveMenu();
    expect(store.entered).toBe(false);
    expect(store.selectedMenuId).toBe("road"); // 保持选中，便于面板回看
  });

  it("updateItem 只做覆盖，未编辑字段回落基础数据", () => {
    const store = makeStore();
    store.updateItem("road", "BBBB0001", { label: "我的道路" });
    const edited = store.activeMenu?.entries[0].tool;
    expect(edited?.label).toBe("我的道路");
    expect(edited?.pos).toBe(10);
  });

  it("addItem 追加到菜单末位；removeItem 可删除新增项", () => {
    const store = makeStore();
    store.enterMenu("road");
    store.addItem("road", "新道路", null);
    const entries = store.activeMenu?.entries ?? [];
    const addedEntry = entries.at(-1);
    expect(addedEntry?.tool.label).toBe("新道路");
    expect(addedEntry?.tool.pos).toBe(30);
    expect(addedEntry?.isNew).toBe(true);

    store.removeItem("road", addedEntry?.tool.id ?? "");
    expect(
      store.activeMenu?.entries.some((entry) => entry.tool.label === "新道路"),
    ).toBe(false);
  });

  it("addCategory 新建分类并选中；removeCategory 可删除", () => {
    const store = makeStore();
    store.addCategory("新分类");
    expect(store.categories.at(-1)?.id).toBe(store.selectedMenuId);
    expect(store.activeMenu?.label).toBe("新分类");
    store.removeCategory(store.selectedMenuId ?? "");
    expect(store.categories.some((c) => c.label === "新分类")).toBe(false);
  });

  it("exportOverlay 携带维度/编辑/新增/自定义分类；resetAll 清空", () => {
    const store = makeStore();
    store.updateItem("road", "BBBB0001", { label: "我的道路" });
    store.addItem("road", "新道路", null);
    store.addCategory("新分类");
    const overlay = JSON.parse(store.exportOverlay());
    expect(overlay.format).toBe("openscp-ui-workbench-overlay/1");
    expect(overlay.edits["BBBB0001"].label).toBe("我的道路");
    expect(overlay.added.road).toHaveLength(1);
    expect(overlay.customCategories).toHaveLength(1);

    store.resetAll();
    expect(Object.keys(store.edits)).toHaveLength(0);
    expect(store.activeMenu?.entries.some((entry) => entry.isNew)).toBe(false);
  });
});
