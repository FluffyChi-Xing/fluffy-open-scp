import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import {
  applyEdit,
  sortTools,
  type ToolEdit,
  type WorkbenchCategory,
  type WorkbenchData,
  type WorkbenchTool,
} from "@/lib/game-ui/workbench";
import {
  resolveLayout,
  type LayoutNode,
  type PlacedImage,
  type PlacedRect,
} from "@/lib/game-ui/scrui";

/**
 * UI 工作台状态：左侧维度切换 + 一级菜单钻入二级 + 菜单项编辑覆盖。
 *
 * 维度（对应游戏左下切换簇）：city = 城市、bigbiz = 大商业、region = 区域。
 * 切换维度自动更新一级菜单内容；点一级分类滑入二级（工具条目）；
 * 编辑是**纯前端覆盖**（`edits` / `added`），实时反映到重建 UI 上；
 * 「落库」当前以 overlay JSON 导出，后端写回（patch_property_overlay +
 * 版本记录通道）是下一轮接入点。
 */
export type WorkbenchDimension = "city" | "bigbiz" | "region";

/** 维度 → 一级分类 id 列表（与 workbench.json 的分类 id 对应）。 */
export const DIMENSION_CATEGORIES: Record<WorkbenchDimension, string[]> = {
  city: [
    "road",
    "power",
    "water",
    "sewage",
    "garbage",
    "fire",
    "health",
    "safety",
    "park",
    "education",
  ],
  bigbiz: ["trade", "landmark", "mayor"],
  region: [],
};

/** 面板里一条菜单项的最终视图（基础数据 + 编辑覆盖 + 新增标记）。 */
export interface MenuEntry {
  menuId: string;
  tool: WorkbenchTool;
  /** 本条是工作台里新加的（尚未有对应的游戏 property）。 */
  isNew?: boolean;
}

export const useUiWorkbenchStore = defineStore("uiWorkbench", () => {
  const data = shallowRef<WorkbenchData | null>(null);
  const loading = shallowRef(false);
  const dimension = ref<WorkbenchDimension>("city");
  /** 当前选中一级分类 id。 */
  const selectedMenuId = ref<string | null>(null);
  /** 是否已钻入二级菜单（一级行 → 二级行的滑动过渡）。 */
  const entered = ref(false);
  /** itemId → 编辑覆盖（含新增项的 id）。 */
  const edits = ref<Record<string, ToolEdit>>({});
  /** menuId → 新增项（保持插入顺序）。 */
  const added = ref<Record<string, WorkbenchTool[]>>({});
  /** 正在编辑的条目（打开 Sheet）。 */
  const editingEntry = shallowRef<MenuEntry | null>(null);
  /** 工作台里新建的分类（一级菜单末位「＋」产生）。 */
  const customCategories = ref<WorkbenchCategory[]>([]);
  /** 游戏 HUD 布局树（scrui JSON）+ 两段式解析结果。 */
  const hudTree = shallowRef<LayoutNode | null>(null);
  const hudRects = shallowRef<Map<string, PlacedRect>>(new Map());
  const hudImages = shallowRef<PlacedImage[]>([]);
  /** 自定义覆盖层排除的装饰件（笑脸精灵由单帧裁切替代）。 */
  const HUD_IMAGE_EXCLUDE = new Set(["1888"]);
  /** 运行时由 JS 填充/无需静态还原的子树（右上社交钮条等）→ 美术层排除。 */
  const HUD_EXCLUDE_SUBTREES = new Set(["1079", "1273", "1364", "1367", "2161"]);

  async function load(): Promise<void> {
    loading.value = true;
    try {
      const response = await fetch("/game-ui/workbench.json");
      data.value = (await response.json()) as WorkbenchData;
      if (!selectedMenuId.value) selectedMenuId.value = firstMenuId();
    } finally {
      loading.value = false;
    }
    // 布局树独立装载：失败只损失美术层，不拖垮菜单数据/编辑
    try {
      const layoutResponse = await fetch("/game-ui/layout/globalui2.json");
      hudTree.value = (await layoutResponse.json()) as LayoutNode;
      const resolved = resolveLayout(hudTree.value, 1600, 900, {
        excludeSubtreeIds: HUD_EXCLUDE_SUBTREES,
      });
      hudRects.value = resolved.rects;
      hudImages.value = resolved.images.filter(
        (image) =>
          !HUD_IMAGE_EXCLUDE.has(image.id) &&
          !/tutorial/i.test(image.comment),
      );
    } catch (cause) {
      console.warn("[uiWorkbench] 布局树解析失败，美术层停用", cause);
    }
  }

  /** 当前维度下的第一个一级分类。 */
  function firstMenuId(): string | null {
    return categories.value[0]?.id ?? null;
  }

  /** 切换左侧维度：一级菜单内容自动更新，二级状态重置。 */
  function selectDimension(next: WorkbenchDimension): void {
    dimension.value = next;
    entered.value = false;
    selectedMenuId.value = firstMenuId();
  }

  /** 一级菜单点击：选中该菜单（面板同步）并滑入二级。 */
  function enterMenu(menuId: string): void {
    selectedMenuId.value = menuId;
    entered.value = true;
  }

  /** 二级返回一级。 */
  function leaveMenu(): void {
    entered.value = false;
  }

  /** 当前维度可见的分类（含工作台新建的）。 */
  const categories = computed<WorkbenchCategory[]>(() => {
    const dimensionIds = DIMENSION_CATEGORIES[dimension.value];
    const all = [...(data.value?.city.categories ?? []), ...customCategories.value];
    if (dimension.value === "city") {
      return all;
    }
    return all.filter((category) => dimensionIds.includes(category.id));
  });

  /** 面板当前展示的菜单（含编辑覆盖与新增项）。 */
  const activeMenu = computed<{ id: string; label: string; entries: MenuEntry[] } | null>(() => {
    if (!data.value) return null;
    const category = categories.value.find(
      (candidate) => candidate.id === selectedMenuId.value,
    );
    if (!category) return null;
    const menuAdditions = added.value[category.id] ?? [];
    const entries: MenuEntry[] = sortTools(
      category.items
        .map((tool) => applyEdit(tool, edits.value[tool.id]))
        .concat(menuAdditions.map((tool) => applyEdit(tool, edits.value[tool.id]))),
    ).map((tool) => ({
      menuId: category.id,
      tool,
      isNew: menuAdditions.some((addedTool) => addedTool.id === tool.id),
    }));
    return { id: category.id, label: category.label, entries };
  });

  /** 面板点击条目 → 打开编辑 Sheet。 */
  function openEditor(menuId: string, itemId: string): void {
    const entry = activeMenu.value?.entries.find(
      (candidate) => candidate.tool.id === itemId,
    );
    if (entry) editingEntry.value = entry;
    void menuId;
  }

  function updateItem(menuId: string, itemId: string, patch: ToolEdit): void {
    edits.value = { ...edits.value, [itemId]: { ...edits.value[itemId], ...patch } };
    void menuId;
  }

  /** 工具条末位「＋」：新建一个空分类并选中。 */
  function addCategory(label: string): void {
    const id = `NEW-CAT-${customCategories.value.length + 1}`;
    customCategories.value = [
      ...customCategories.value,
      { id, label, icon: null, items: [] },
    ];
    selectedMenuId.value = id;
  }

  /** 删除工作台新建的分类。 */
  function removeCategory(id: string): void {
    customCategories.value = customCategories.value.filter((c) => c.id !== id);
    const next = { ...added.value };
    delete next[id];
    added.value = next;
    if (selectedMenuId.value === id) {
      selectedMenuId.value = firstMenuId();
    }
  }

  /** 菜单末位追加一项：id 取 next-<n>，排序键排在当前最大值之后。 */
  function addItem(menuId: string, label: string, icon: string | null): void {
    const baseItems =
      menuId === "university"
        ? (data.value?.university.tools ?? [])
        : (categories.value.find((category) => category.id === menuId)?.items ?? []);
    const existing = [...baseItems, ...(added.value[menuId] ?? [])];
    const maxPos = existing.reduce((max, tool) => Math.max(max, tool.pos), 0);
    const id = `NEW-${menuId}-${(added.value[menuId]?.length ?? 0) + 1}`;
    const tool: WorkbenchTool = {
      id,
      label,
      pos: maxPos + 10,
      icon,
      source: "unresolved",
    };
    added.value = { ...added.value, [menuId]: [...(added.value[menuId] ?? []), tool] };
    edits.value = { ...edits.value, [id]: { ...edits.value[id] } };
  }

  function removeItem(menuId: string, itemId: string): void {
    if (added.value[menuId]?.some((tool) => tool.id === itemId)) {
      added.value = {
        ...added.value,
        [menuId]: added.value[menuId].filter((tool) => tool.id !== itemId),
      };
    }
    const next = { ...edits.value };
    delete next[itemId];
    edits.value = next;
    if (editingEntry.value?.tool.id === itemId) editingEntry.value = null;
  }

  /** 落库前的当前覆盖（导出 JSON；真实写回走下一轮的 overlay 通道）。 */
  function exportOverlay(): string {
    return JSON.stringify(
      {
        format: "openscp-ui-workbench-overlay/1",
        dimension: dimension.value,
        edits: edits.value,
        added: added.value,
        customCategories: customCategories.value,
      },
      null,
      2,
    );
  }

  function resetAll(): void {
    edits.value = {};
    added.value = {};
    editingEntry.value = null;
  }

  /** 布局树某节点的运行期绝对矩形（instanceID 十进制字符串）。 */
  function rect(id: string): PlacedRect | undefined {
    return hudRects.value.get(id);
  }

  return {
    data,
    loading,
    dimension,
    selectedMenuId,
    entered,
    edits,
    added,
    editingEntry,
    customCategories,
    hudTree,
    hudRects,
    hudImages,
    categories,
    activeMenu,
    rect,
    load,
    selectDimension,
    enterMenu,
    leaveMenu,
    openEditor,
    addCategory,
    removeCategory,
    updateItem,
    addItem,
    removeItem,
    exportOverlay,
    resetAll,
  };
});
