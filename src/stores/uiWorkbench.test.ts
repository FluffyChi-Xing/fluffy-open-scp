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
        { id: "3193C0BE", label: "區域", icon: null, iconHash: null },
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
    expect(store.categories.map((category) => category.id)).toEqual(["CAT_ROAD", "CAT_PARK", "3193C0BE"]);
    store.enterMenu("CAT_ROAD");
    expect(store.activeMenu?.label).toBe("道路");
    expect(store.activeMenu?.entries.map((entry) => entry.tool.id)).toEqual([
      "BBBB0001",
      "BBBB0002",
      "BBBB0003",
    ]);
  });

  it("keeps zoning in city view and uses separate region menu groups", () => {
    const store = makeStore();
    expect(store.categories.some((category) => category.id === "3193C0BE")).toBe(true);
    store.setViewMode("region");
    expect(store.categories.some((category) => category.id === "3193C0BE")).toBe(false);
    expect(store.categories.map((category) => category.id)).toContain("REGION-TRANSPORT");
    store.setViewMode("city");
    expect(store.categories.some((category) => category.id === "3193C0BE")).toBe(true);
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
    expect(overlay.format).toBe("openscp-ui-workbench-overlay/2");
    expect(overlay.edits["BBBB0001"].label).toBe("我的道路");
    expect(overlay.added.CAT_ROAD).toHaveLength(1);
    expect(overlay.customCategories).toHaveLength(1);

    store.resetAll();
    expect(Object.keys(store.edits)).toHaveLength(0);
    expect(store.activeMenu?.entries.some((entry) => entry.isNew)).toBe(false);
  });

  it("deleting a built-in entry removes it from the active menu and schema", () => {
    const store = makeStore();
    store.enterMenu("CAT_ROAD");
    store.removeItem("CAT_ROAD", "BBBB0001");
    expect(store.activeMenu?.entries.map((entry) => entry.tool.id)).not.toContain("BBBB0001");
    const schema = JSON.parse(store.exportSchema());
    expect(schema.menus[0].items.map((item: { id: string }) => item.id)).not.toContain("BBBB0001");
  });

  it("图片三态：移除大图后有效视图为 null，不再回落基础资产", () => {
    const store = makeStore();
    const data = replicaFixture();
    data.tools["CAT_ROAD"][0] = {
      ...data.tools["CAT_ROAD"][0],
      marquee: "/game-ui/replica/assets/tool_BBBB0001.jpg",
    };
    store.hydrate(data);
    store.enterMenu("CAT_ROAD");
    expect(store.activeMenu?.entries[0].tool.marquee).toContain("tool_BBBB0001.jpg");
    store.updateItem("CAT_ROAD", "BBBB0001", { marquee: null });
    expect(store.activeMenu?.entries[0].tool.marquee).toBeNull();
  });

  it("duplicateToMenu：新身份副本快照源条目，与源互不影响", () => {
    const store = makeStore();
    store.enterMenu("CAT_ROAD");
    store.updateItem("CAT_ROAD", "BBBB0001", { label: "改名后的泥路" });
    const copy = store.duplicateToMenu("CAT_ROAD", "BBBB0001", "CAT_PARK");
    expect(copy).not.toBeNull();
    expect(copy?.menuId).toBe("CAT_PARK");
    expect(copy?.isNew).toBe(true);
    expect(copy?.tool.id).not.toBe("BBBB0001");
    expect(copy?.tool.label).toBe("改名后的泥路"); // 快照含已有编辑
    expect(copy?.tool.pos).toBe(10); // 目标菜单当前为空 → 末位

    // 拷贝后源继续编辑，副本不受影响（编辑只落在各自覆盖键上）
    store.updateItem("CAT_ROAD", "BBBB0001", { label: "又改了" });
    expect(store.activeMenu?.entries[0].tool.label).toBe("又改了");
    store.enterMenu("CAT_PARK");
    expect(store.activeMenu?.entries[0].tool.label).toBe("改名后的泥路");
    expect(store.activeMenu?.entries[0].tool.id).toBe(copy?.tool.id);

    // 重复拷贝到同一菜单：id 不冲突
    const second = store.duplicateToMenu("CAT_ROAD", "BBBB0001", "CAT_PARK");
    expect(second?.tool.id).not.toBe(copy?.tool.id);
  });

  it("membershipsOf 列出基础归属与新增归属", () => {
    const store = makeStore();
    store.duplicateToMenu("CAT_ROAD", "BBBB0001", "CAT_PARK");
    const memberships = store.membershipsOf("BBBB0001");
    expect(memberships.map((menu) => menu.id)).toEqual(["CAT_ROAD"]);
    const copyId = (store.added["CAT_PARK"] ?? [])[0]?.id ?? "";
    expect(store.membershipsOf(copyId).map((menu) => menu.id)).toEqual(["CAT_PARK"]);
  });

  it("buildGameExport 基于有效视图：只导编辑/新增条目，归属容器为真实 id", () => {
    const store = useUiWorkbenchStore();
    const data = replicaFixture();
    // 真实容器 id（AC12C904 道路）+ 真实条目大图资产
    data.model.categories[0] = { id: "AC12C904", label: "道路", icon: null, iconHash: null };
    data.tools["AC12C904"] = [
      {
        instance: "7DD8A627",
        label: "低密度泥路",
        preview: "/game-ui/replica/assets/tool_icon_2531E3C9.png",
        marquee: "/game-ui/replica/assets/tool_7DD8A627.jpg",
        locked: false,
      },
      { instance: "B7D3F194", label: "低密道街道", preview: null, marquee: null, locked: false },
    ];
    delete data.tools["CAT_ROAD"];
    store.hydrate(data);
    store.enterMenu("AC12C904");

    // 无编辑 → 不导出任何条目
    expect(store.buildGameExport().entryCount).toBe(0);

    // 编辑一个条目（含造价）→ 只导出它；造价警告说明 GlassBox 侧不进包
    store.updateItem("AC12C904", "7DD8A627", { label: "我的泥路", cost: "20" });
    const result = store.buildGameExport();
    expect(result.entryCount).toBe(1);
    expect(result.warnings.join()).toContain("GlassBox");

    // 移除大图 → 仍然导出（其它编辑保留），但不再有大图悬空引用
    store.updateItem("AC12C904", "7DD8A627", { marquee: null });
    expect(store.buildGameExport().entryCount).toBe(1);

    // 拷贝到大学容器（9D2EF585，真实 id）→ 新条目以该容器为归属导出
    const copy = store.duplicateToMenu("AC12C904", "7DD8A627", "9D2EF585");
    expect(copy).not.toBeNull();
    const withCopy = store.buildGameExport();
    expect(withCopy.entryCount).toBe(2);

    // 非游戏容器（自定义分类）里的新增条目给出可读警告且不进包
    store.addCategory("我的菜单");
    const customId = store.categories.at(-1)?.id ?? "";
    store.addItem(customId, "自定义条目", null);
    const withCustom = store.buildGameExport();
    expect(withCustom.entryCount).toBe(2);
    expect(withCustom.warnings.join()).toContain("不是游戏容器 instance");
  });

  it("development 视图使用真实大学容器 id（9D2EF585）", () => {
    const store = makeStore();
    store.setViewMode("development");
    expect(store.categories[0]?.id).toBe("9D2EF585");
  });
});
