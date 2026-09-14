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
    store.selectScreen("city");
    return store;
  }

  it("selectScreen 默认选中第一个分类；面板列出该分类条目", () => {
    const store = makeStore();
    expect(store.screen).toBe("city");
    expect(store.selectedMenuId).toBe("road");
    expect(store.activeMenu?.label).toBe("道路");
    expect(store.activeMenu?.entries.map((entry) => entry.tool.id)).toEqual([
      "BBBB0001",
      "BBBB0002",
    ]);
  });

  it("切换到大学屏幕后面板列出大学菜单（按 pos 排序）", () => {
    const store = makeStore();
    store.selectScreen("university");
    expect(store.activeMenu?.id).toBe("university");
    expect(store.activeMenu?.entries.map((entry) => entry.tool.id)).toEqual([
      "AAAA0001",
      "AAAA0002",
    ]);
  });

  it("updateItem 只做覆盖，未编辑字段回落基础数据", () => {
    const store = makeStore();
    store.updateItem("road", "BBBB0001", { label: "我的道路" });
    const edited = store.activeMenu?.entries[0].tool;
    expect(edited?.label).toBe("我的道路");
    expect(edited?.pos).toBe(10);
    expect(edited?.source).toBe("locale");
  });

  it("addItem 追加到菜单末位并按 pos 排序；removeItem 可删除新增项", () => {
    const store = makeStore();
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

  it("addCategory 新建分类并选中，工具条与面板同步出现", () => {
    const store = makeStore();
    store.addCategory("新分类");
    expect(store.categories.at(-1)?.id).toBe(store.selectedMenuId);
    expect(store.activeMenu?.label).toBe("新分类");
    expect(store.activeMenu?.entries).toEqual([]);
  });

  it("enterMenu 钻入二级、leaveMenu 返回、切屏重置", () => {
    const store = makeStore();
    expect(store.entered).toBe(false);
    store.enterMenu("road");
    expect(store.entered).toBe(true);
    expect(store.selectedMenuId).toBe("road");
    store.leaveMenu();
    expect(store.entered).toBe(false);
    store.enterMenu("power");
    store.selectScreen("university");
    expect(store.entered).toBe(false);
    expect(store.selectedMenuId).toBe("university");
  });

  it("exportOverlay 携带编辑、新增与自定义分类；resetAll 清空", () => {
    const store = makeStore();
    store.updateItem("road", "BBBB0001", { label: "我的道路" });
    store.addItem("university", "新学院", null);
    store.addCategory("新分类");
    const overlay = JSON.parse(store.exportOverlay());
    expect(overlay.format).toBe("openscp-ui-workbench-overlay/1");
    expect(overlay.edits["BBBB0001"].label).toBe("我的道路");
    expect(overlay.added.university).toHaveLength(1);
    expect(overlay.customCategories).toHaveLength(1);

    store.resetAll();
    expect(Object.keys(store.edits)).toHaveLength(0);
    expect(store.activeMenu?.entries.some((entry) => entry.isNew)).toBe(false);
  });
});
