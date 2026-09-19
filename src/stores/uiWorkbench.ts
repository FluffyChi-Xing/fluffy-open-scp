import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import {
  applyEdit,
  sortTools,
  type ToolEdit,
  type WorkbenchCategory,
  type WorkbenchTool,
} from "@/lib/game-ui/workbench";
import { loadReplicaData } from "@/lib/game-ui/replica/loader";
import type { ReplicaData, ReplicaTool } from "@/lib/game-ui/replica/types";

/**
 * UI 工作台状态：一级菜单 → 二级面板钻入 + 菜单项编辑覆盖。
 *
 * 数据源 = ui_replica 复刻数据（public/game-ui/replica/data/*.js，逆向产物生成）：
 * 一级 14 个分类来自 CATEGORY_MODEL（Menu/Menu2 推导），二级槽位来自 HUD_TOOLS，
 * 面板布局来自 HUD_PALETTE；舞台渲染（Shadow DOM）在 UiWorkbenchStage 里完成，
 * store 只维护菜单模型与**纯前端编辑覆盖**（`edits` / `added`），实时反映到
 * 复刻 UI 与右侧面板上；「落库」当前以 overlay JSON 导出，后端写回
 * （patch_property_overlay + 版本记录通道）是下一轮接入点。
 */

/** 面板里一条菜单项的最终视图（基础数据 + 编辑覆盖 + 新增标记）。 */
export interface MenuEntry {
  menuId: string;
  tool: WorkbenchTool;
  /** 本条是工作台里新加的（尚未有对应的游戏 property）。 */
  isNew?: boolean;
}

/** 复刻数据里的一个工具 → 工作台的条目视图。
 * 顺序即 uiPosition（构建期已排序），映射为 10/20/30… 的排序键；
 * 名字缺失时回退 instance 占位（source=unresolved）。
 * `icon` 只承载用户覆盖（FIcon 名）；真实槽位图走 `preview`（kPropToolIconKey
 * 提取物），desc/unlock/locked 供 hover 提示框使用。 */
export function replicaToolToWorkbench(tool: ReplicaTool, index: number): WorkbenchTool {
  return {
    id: tool.instance,
    label: tool.label || tool.instance,
    pos: (index + 1) * 10,
    icon: null,
    preview: tool.preview,
    marquee: tool.marquee,
    desc: tool.desc,
    unlock: tool.unlock,
    locked: tool.locked,
    source: tool.label ? "locale" : "unresolved",
  };
}

export const useUiWorkbenchStore = defineStore("uiWorkbench", () => {
  /** ui_replica 复刻数据（菜单模型 + 面板布局 + 舞台渲染的输入）。 */
  const replica = shallowRef<ReplicaData | null>(null);
  const loading = shallowRef(false);
  const loadError = shallowRef<string | null>(null);
  /** 当前选中一级分类 id。 */
  const selectedMenuId = ref<string | null>(null);
  /** 是否已进入二级（面板打开）。 */
  const entered = ref(false);
  /** itemId → 编辑覆盖（含新增项的 id）。 */
  const edits = ref<Record<string, ToolEdit>>({});
  /** menuId → 新增项（保持插入顺序）。 */
  const added = ref<Record<string, WorkbenchTool[]>>({});
  /** 正在编辑的条目（打开 Sheet）。 */
  const editingEntry = shallowRef<MenuEntry | null>(null);
  /** 工作台里新建的分类（一级菜单末位「＋」产生）。 */
  const customCategories = ref<WorkbenchCategory[]>([]);

  /** 从复刻数据脚本装载（幂等；数据进程内缓存）。 */
  async function load(): Promise<void> {
    loading.value = true;
    loadError.value = null;
    try {
      const data = await loadReplicaData();
      await backfillParkTools(data);
      hydrate(data);
    } catch (cause) {
      loadError.value = String(cause);
      console.warn("[uiWorkbench] 复刻数据装载失败", cause);
    } finally {
      loading.value = false;
    }
  }

  /** 公园分类的工具条目在逆向产物里缺失（Game/App/EP1/DLC0 四包都无对应
   * Menu 条目），从旧工作台数据链 workbench.json 回填——重构前公园可以
   * 解析出菜单项，行为保持一致。预览资源旧链同样没有 → 槽位走占位图。 */
  async function backfillParkTools(data: ReplicaData): Promise<void> {
    const parkCategory =
      data.model.categories.find((c) => c.label === "公園") ??
      data.model.categories.find((c) => c.id === "899BEA70");
    if (!parkCategory || (data.tools[parkCategory.id]?.length ?? 0) > 0) return;
    try {
      const response = await fetch("/game-ui/workbench.json");
      const legacy = (await response.json()) as {
        city?: { categories?: { id: string; items?: { id: string; label: string }[] }[] };
      };
      const park = legacy.city?.categories?.find((category) => category.id === "park");
      if (!park?.items?.length) return;
      data.tools[parkCategory.id] = park.items.map((item) => ({
        instance: item.id,
        label: item.label,
        preview: null,
        marquee: null,
        locked: false,
      }));
    } catch (cause) {
      console.warn("[uiWorkbench] 公园菜单项回填失败", cause);
    }
  }

  /** 注入复刻数据（load 的实质步骤；测试直接调用以绕开脚本注入）。
   * 不默认选中任何菜单——未选中时右侧面板显示 FEmpty 占位。 */
  function hydrate(data: ReplicaData): void {
    replica.value = data;
    if (selectedMenuId.value && !categories.value.some((c) => c.id === selectedMenuId.value)) {
      selectedMenuId.value = null;
      entered.value = false;
    }
  }

  /** 一级菜单点击：选中该分类并进入二级（面板由舞台打开；再点同一个 =
   * 收起，由舞台根据面板开关状态调用 enterMenu/leaveMenu）。 */
  function enterMenu(menuId: string): void {
    selectedMenuId.value = menuId;
    entered.value = true;
  }

  /** 二级返回一级。 */
  function leaveMenu(): void {
    entered.value = false;
  }

  /** 全部一级分类（复刻数据 14 分类 + 工作台新建的）。 */
  const categories = computed<WorkbenchCategory[]>(() => {
    const data = replica.value;
    const base = (data?.model.categories ?? []).map((cat) => ({
      id: cat.id,
      label: cat.label,
      icon: cat.icon,
      items: (data?.tools[cat.id] ?? []).map(replicaToolToWorkbench),
    }));
    return [...base, ...customCategories.value];
  });

  /** 面板当前展示的菜单（含编辑覆盖与新增项）。 */
  const activeMenu = computed<{ id: string; label: string; entries: MenuEntry[] } | null>(() => {
    if (!replica.value) return null;
    const category = categories.value.find((candidate) => candidate.id === selectedMenuId.value);
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
    const entry = activeMenu.value?.entries.find((candidate) => candidate.tool.id === itemId);
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
      selectedMenuId.value = null;
      entered.value = false;
    }
  }

  /** 菜单末位追加一项：id 取 next-<n>，排序键排在当前最大值之后。 */
  function addItem(menuId: string, label: string, icon: string | null): void {
    const baseItems = categories.value.find((category) => category.id === menuId)?.items ?? [];
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

  return {
    replica,
    loading,
    loadError,
    selectedMenuId,
    entered,
    edits,
    added,
    editingEntry,
    customCategories,
    categories,
    activeMenu,
    hydrate,
    load,
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
