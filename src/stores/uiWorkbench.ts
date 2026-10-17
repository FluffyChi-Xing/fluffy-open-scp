import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import {
  applyEdit,
  fnv1Lower,
  sortTools,
  type ToolEdit,
  type WorkbenchCategory,
  type WorkbenchTool,
} from "@/lib/game-ui/workbench";
import {
  buildMenuExport,
  buildMenuPropertyResource,
  dataUrlToBytes,
  iconInstanceFromAsset,
  LOCALE_TABLES,
  marqueeInstanceFromAsset,
  type LocaleString,
  type MenuExportEntry,
} from "@/lib/game-ui/menu-export";
import { loadReplicaData } from "@/lib/game-ui/replica/loader";
import type { ReplicaData, ReplicaSubmenuItem, ReplicaTool } from "@/lib/game-ui/replica/types";
import { i18n } from "@/locales";

export type UiWorkbenchView = "city" | "region" | "development";

interface LegacyWorkbench {
  city?: { categories?: { id: string; items?: { id: string; label: string; pos: number }[] }[] };
  university?: {
    label: string;
    tools?: { id: string; label: string; pos: number; icon?: string | null }[];
  };
}

/**
 * UI 工作台状态：一级菜单 → 二级面板钻入 + 菜单项编辑覆盖。
 *
 * 数据源 = ui_replica 复刻数据（public/game-ui/replica/data/*.js，逆向产物生成）：
 * 一级 14 个分类来自 CATEGORY_MODEL（Menu/Menu2 推导），二级槽位来自 HUD_TOOLS，
 * 面板布局来自 HUD_PALETTE；舞台渲染（Shadow DOM）在 UiWorkbenchStage 里完成。
 *
 * 副本语义：复刻基础数据只读；一切编辑落在 `edits`（逐字段覆盖，三态——
 * undefined 不改 / null 明确移除 / 字符串替换）、`added`（新增与拷贝条目）、
 * `removed`（移出菜单）三张覆盖表。面板/舞台/导出共用 effectiveEntries
 * 的同一有效模型；`duplicateToMenu` 实现条目拷贝（新身份 + 共享原图）。
 * 「落库」当前以 overlay JSON / buildGameExport 的 .package 导出，后端写回
 * （patch_property_overlay + 版本记录通道）是下一轮接入点。
 */

/** 面板里一条菜单项的最终视图（基础数据 + 编辑覆盖 + 新增标记）。 */
export interface MenuEntry {
  menuId: string;
  tool: WorkbenchTool;
  /** 本条是工作台里新加的（尚未有对应的游戏 property）。 */
  isNew?: boolean;
  /** 本条来自建筑子菜单（costs.json 反查产物），不在任何工作台分类里。 */
  isSubmenu?: boolean;
}

/** 菜单容器 instance（0x%08X）：游戏数据里的「归属」由 0x0DB9FC63
 * （uiCategories）数组决定——条目 property 指向容器 instance，一个条目
 * 可以同时挂在多个菜单下。工作台三类视图对应的真实容器：
 * - 城市（city）= 复刻数据的 14 个一级分类（AC12C904 道路…）；
 * - 城市发展（development）= 大学子菜单容器 9D2EF585（8 个成员：人行道、
 *   大學標誌、宿舍、四大学院；与 tools.json 的 uiCategories 反查完全一致）；
 * - 区域（region）= RegionUI 布局的按钮组由运行时填充，静态包无容器
 *   instance → 保持占位 id，不参与编译导出。 */
export const UNIVERSITY_MENU_CONTAINER = "9D2EF585";

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
    stats: tool.stats,
    cost: tool.cost,
    upkeep: tool.upkeep,
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
  const removed = ref<Record<string, string[]>>({});
  /** 正在编辑的条目（打开 Sheet）。 */
  const editingEntry = shallowRef<MenuEntry | null>(null);
  /** Sheet 嵌套栈：子菜单条目入栈，关闭时弹回上级。 */
  const editorStack = shallowRef<MenuEntry[]>([]);
  /** 工作台里新建的分类（一级菜单末位「＋」产生）。 */
  const customCategories = ref<WorkbenchCategory[]>([]);
  const viewMode = ref<UiWorkbenchView>("city");
  const legacyWorkbench = shallowRef<LegacyWorkbench | null>(null);

  /** 从复刻数据脚本装载（幂等；数据进程内缓存）。 */
  async function load(): Promise<void> {
    loading.value = true;
    loadError.value = null;
    try {
      const data = await loadReplicaData();
      try {
        const response = await fetch("/game-ui/workbench.json");
        legacyWorkbench.value = (await response.json()) as LegacyWorkbench;
      } catch {
        legacyWorkbench.value = null;
      }
      hydrate(data);
    } catch (cause) {
      loadError.value = String(cause);
      console.warn("[uiWorkbench] 复刻数据装载失败", cause);
    } finally {
      loading.value = false;
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

  function setViewMode(mode: UiWorkbenchView): void {
    viewMode.value = mode;
    entered.value = false;
    selectedMenuId.value = categories.value[0]?.id ?? null;
  }

  /** 二级返回一级。 */
  function leaveMenu(): void {
    entered.value = false;
  }

  /** 大学容器的条目（真实 instance id；与 tools.json 里 uiCategories=9D2EF585
   * 的 8 个成员一致）。城市发展视图的 categories 与跨视图拷贝共用。 */
  const universityItems = computed<WorkbenchTool[]>(() =>
    (legacyWorkbench.value?.university?.tools ?? []).map((tool) => ({
      id: tool.id,
      label: tool.label || tool.id,
      pos: tool.pos,
      icon: tool.icon ?? null,
      source: "locale" as const,
    })),
  );

  function universityLabel(): string {
    return legacyWorkbench.value?.university?.label ?? i18n.global.t("studio.workbench.universityMenu");
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
    if (viewMode.value === "region") {
      const tr = i18n.global.t;
      const regionCategories: WorkbenchCategory[] = [
        ["REGION-GREAT-WORKS", "regionGreatWorks"],
        ["REGION-COMMUTERS", "regionCommuters"],
        ["REGION-TRANSPORT", "regionTransport"],
        ["REGION-UTILITIES", "regionUtilities"],
        ["REGION-SERVICES", "regionServices"],
        ["REGION-GIFTS", "regionGifts"],
      ].map(([id, key]) => ({ id, label: tr(`studio.workbench.${key}`), icon: null, items: [] }));
      return [...regionCategories, ...customCategories.value];
    }
    if (viewMode.value === "development") {
      return [
        {
          // 真实菜单容器 instance：大学子菜单（0x0DB9FC63 归属键的目标值）。
          // 用真 id 而非占位符，拷贝/新增条目才能编译出游戏可读的归属。
          id: UNIVERSITY_MENU_CONTAINER,
          label: legacyWorkbench.value?.university?.label ?? "城市发展",
          icon: null,
          items: universityItems.value,
        },
        ...customCategories.value,
      ];
    }
    return [...base, ...customCategories.value];
  });

  /** 面板当前展示的菜单（含编辑覆盖与新增项）。 */
  const activeMenu = computed<{ id: string; label: string; entries: MenuEntry[] } | null>(() => {
    if (!replica.value) return null;
    const category = categories.value.find((candidate) => candidate.id === selectedMenuId.value);
    if (!category) return null;
    return { id: category.id, label: category.label, entries: effectiveEntries(category).entries };
  });

  /** 一个菜单的**有效视图** = 基础条目 − 已移除 + 已新增，全部应用编辑覆盖。
   * 面板、舞台、导出共用这一个模型——「编辑器编辑的是副本」：基础复刻数据
   * 只读，所有修改都落在 `edits` / `added` / `removed` 三张覆盖表里。 */
  function effectiveEntries(category: WorkbenchCategory): { category: WorkbenchCategory; entries: MenuEntry[] } {
    const menuAdditions = added.value[category.id] ?? [];
    const dropped = removed.value[category.id] ?? [];
    const entries: MenuEntry[] = sortTools(
      category.items
        .filter((tool) => !dropped.includes(tool.id))
        .map((tool) => applyEdit(tool, edits.value[tool.id]))
        .concat(menuAdditions.map((tool) => applyEdit(tool, edits.value[tool.id]))),
    ).map((tool) => ({
      menuId: category.id,
      tool,
      isNew: menuAdditions.some((addedTool) => addedTool.id === tool.id),
    }));
    return { category, entries };
  }

  /** 全部菜单的有效视图（导出的唯一数据源）。跨视图容器（大学
   * 9D2EF585）不在当前视图分类里，但有新增/拷贝条目时同样参与导出。 */
  function allEffectiveEntries(): { category: WorkbenchCategory; entries: MenuEntry[] }[] {
    const out = categories.value.map((category) => effectiveEntries(category));
    for (const [menuId, additions] of Object.entries(added.value)) {
      if (additions.length === 0) continue;
      if (out.some(({ category }) => category.id === menuId)) continue;
      const items = menuId === UNIVERSITY_MENU_CONTAINER ? universityItems.value : [];
      const label = menuId === UNIVERSITY_MENU_CONTAINER
        ? universityLabel()
        : additions[0]?.label ?? menuId;
      out.push(effectiveEntries({ id: menuId, label, icon: null, items }));
    }
    return out;
  }

  /** 面板点击条目 → 打开编辑 Sheet（已有打开中的 Sheet 时压栈，形成嵌套）。 */
  function openEditor(menuId: string, itemId: string): void {
    const entry = activeMenu.value?.entries.find((candidate) => candidate.tool.id === itemId);
    if (entry) {
      if (editingEntry.value) {
        editorStack.value = [...editorStack.value, editingEntry.value];
      }
      editingEntry.value = entry;
    }
    void menuId;
  }

  /** 打开建筑子菜单条目（costs.json 反查产物；不挂在任何工作台分类）。 */
  function openSubmenuItem(item: ReplicaSubmenuItem): void {
    if (!editingEntry.value) return;
    editorStack.value = [...editorStack.value, editingEntry.value];
    editingEntry.value = {
      menuId: editingEntry.value.menuId,
      isSubmenu: true,
      tool: {
        id: item.instance,
        label: item.name,
        pos: 0,
        icon: null,
        desc: item.desc || undefined,
        preview: item.preview ?? null,
        marquee: item.marquee ?? null,
        cost: item.cost != null ? String(item.cost) : null,
        upkeep: item.maintenance != null ? String(item.maintenance) : null,
        source: "locale",
      },
    };
  }

  /** 关闭当前 Sheet 层：有上级时弹回，否则收起。 */
  function closeEditor(): void {
    if (editorStack.value.length > 0) {
      const stack = [...editorStack.value];
      editingEntry.value = stack.pop() ?? null;
      editorStack.value = stack;
    } else {
      editingEntry.value = null;
    }
  }

  /** 编辑会话整体收起（还原栈）。 */
  function resetEditor(): void {
    editingEntry.value = null;
    editorStack.value = [];
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

  /** 将已核对的大学人行道样例挂到道路菜单，作为流程工作台的演示入口。 */
  function addUniversitySidewalkToRoad(): void {
    const road = categories.value.find((category) =>
      category.items.some((tool) => tool.id === "B7D3F194" || tool.id === "7DD8A627"),
    );
    if (!road) return;
    if ([...road.items, ...(added.value[road.id] ?? [])].some((tool) => tool.id === "9A38E382")) {
      selectedMenuId.value = road.id;
      entered.value = true;
      return;
    }
    const tool: WorkbenchTool = {
      id: "9A38E382",
      label: i18n.global.t("studio.workbench.universitySidewalk"),
      pos: Math.max(...road.items.map((item) => item.pos), 0) + 10,
      icon: null,
      preview: "/game-ui/replica/assets/tool_placeholder.png",
      marquee: "/game-ui/reference/university.png",
      desc: i18n.global.t("studio.workbench.universitySidewalkDesc"),
      unlock: "",
      stats: [
        { label: i18n.global.t("studio.workbench.laneCount"), value: "2" },
        { label: i18n.global.t("studio.workbench.maxDensity"), value: i18n.global.t("studio.workbench.mediumDensity") },
      ],
      // The game item is free by default. The copy is immediately edited to
      // the road-menu sample price below, preserving the preset semantics.
      cost: null,
      upkeep: null,
      locked: false,
      source: "preset",
    };
    added.value = { ...added.value, [road.id]: [...(added.value[road.id] ?? []), tool] };
    edits.value = { ...edits.value, [tool.id]: { cost: "20", upkeep: null } };
    selectedMenuId.value = road.id;
    entered.value = true;
  }

  /** 把一条菜单项**拷贝**进目标菜单（SCP 教程 Copy Entry 的语义：
   * 复制完整条目、分配新身份，原图标/大图先共享原资源）。
   * 副本快照源的当前有效视图（含已有编辑），此后两边互不影响；
   * 编辑/移除都只落在副本自己的覆盖键上。目标可以是当前视图里的分类，
   * 也可以是跨视图的真实容器（大学 9D2EF585）。返回副本条目。 */
  function duplicateToMenu(sourceMenuId: string, itemId: string, targetMenuId: string): MenuEntry | null {
    const source = categories.value.find((category) => category.id === sourceMenuId);
    if (!source) return null;
    const targetInView = categories.value.find((category) => category.id === targetMenuId);
    if (!targetInView && targetMenuId !== UNIVERSITY_MENU_CONTAINER) return null;
    const base = source.items.find((tool) => tool.id === itemId)
      ?? (added.value[sourceMenuId] ?? []).find((tool) => tool.id === itemId);
    if (!base) return null;
    const snapshot = applyEdit(base, edits.value[itemId]);

    const targetItems = targetInView?.items ?? universityItems.value;
    const targetAdditions = added.value[targetMenuId] ?? [];
    const existing = [...targetItems, ...targetAdditions];
    let salt = 0;
    let copyId = copyToolId(itemId, targetMenuId, salt);
    while (existing.some((tool) => tool.id === copyId)) {
      copyId = copyToolId(itemId, targetMenuId, ++salt);
    }
    const maxPos = existing.reduce((max, tool) => Math.max(max, tool.pos), 0);
    const copy: WorkbenchTool = { ...snapshot, id: copyId, pos: maxPos + 10 };
    added.value = { ...added.value, [targetMenuId]: [...targetAdditions, copy] };
    return { menuId: targetMenuId, tool: copy, isNew: true };
  }

  /** 副本的工作台 id：可读（COPY-<源id>-<目标>-<n>）；编译时按非游戏
   * instance 走 FNV 分配新身份，与源条目互不覆盖。 */
  function copyToolId(sourceId: string, targetMenuId: string, salt: number): string {
    const suffix = salt === 0 ? "" : `-${salt}`;
    return `COPY-${sourceId}-${targetMenuId}${suffix}`;
  }

  /** 可作为复制目标的菜单全集：当前视图分类（排除区域占位与自身）+
   * 跨视图的大学容器。区域容器没有游戏 instance，不提供。 */
  function copyTargetMenus(excludeMenuId: string): { id: string; label: string }[] {
    const out = new Map<string, string>();
    for (const category of categories.value) {
      if (category.id !== excludeMenuId && !category.id.startsWith("REGION-")) {
        out.set(category.id, category.label);
      }
    }
    if (!out.has(UNIVERSITY_MENU_CONTAINER)) {
      out.set(UNIVERSITY_MENU_CONTAINER, universityLabel());
    }
    return [...out].map(([id, label]) => ({ id, label }));
  }

  /** 条目当前归属的全部菜单（指哪打哪的依据）：复刻数据里出现该 instance
   * 的分类 + 工作台把它加进过的分类 + 跨视图的大学容器。 */
  function membershipsOf(itemId: string): { id: string; label: string }[] {
    const data = replica.value;
    const out: { id: string; label: string }[] = [];
    const seenIds = new Set<string>();
    const menus: { id: string; label: string; items: WorkbenchTool[] }[] = categories.value.map(
      (category) => ({ id: category.id, label: category.label, items: category.items }),
    );
    if (!menus.some((menu) => menu.id === UNIVERSITY_MENU_CONTAINER)) {
      menus.push({ id: UNIVERSITY_MENU_CONTAINER, label: universityLabel(), items: universityItems.value });
    }
    for (const menu of menus) {
      if (seenIds.has(menu.id)) continue;
      seenIds.add(menu.id);
      const inBase = (data?.tools[menu.id] ?? []).some((tool) => tool.instance === itemId)
        || menu.items.some((tool) => tool.id === itemId);
      const inAdded = (added.value[menu.id] ?? []).some((tool) => tool.id === itemId);
      if (inBase || inAdded) out.push({ id: menu.id, label: menu.label });
    }
    return out;
  }

  function removeItem(menuId: string, itemId: string): void {
    if (added.value[menuId]?.some((tool) => tool.id === itemId)) {
      added.value = {
        ...added.value,
        [menuId]: added.value[menuId].filter((tool) => tool.id !== itemId),
      };
    } else {
      removed.value = {
        ...removed.value,
        [menuId]: [...new Set([...(removed.value[menuId] ?? []), itemId])],
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
        format: "openscp-ui-workbench-overlay/2",
        viewMode: viewMode.value,
        edits: edits.value,
        added: added.value,
        customCategories: customCategories.value,
      },
      null,
      2,
    );
  }

  function exportSchema(): string {
    return JSON.stringify(
      {
        schemaVersion: 1,
        source: "openscp-ui-workbench",
        activeView: viewMode.value,
        menus: allEffectiveEntries().map(({ category, entries }) => ({
          id: category.id,
          label: category.label,
          icon: category.icon,
          items: entries.map(({ tool }) => ({
            id: tool.id,
            label: tool.label,
            position: tool.pos,
            icon: tool.icon,
            preview: tool.preview ?? null,
            marquee: tool.marquee ?? null,
            description: tool.desc ?? "",
            unlock: tool.unlock ?? "",
            stats: tool.stats ?? [],
            cost: tool.cost ?? null,
            upkeep: tool.upkeep ?? null,
            locked: tool.locked ?? false,
          })),
        })),
      },
      null,
      2,
    );
  }

  /** 装配游戏覆盖包（.package）：内存编辑 → diff 级覆盖资源
   * （菜单条目 property + locale 表 diff + 裁切图标/大图）。
   * 数据源 = allEffectiveEntries()（与面板/舞台同一有效模型）：
   * 编辑值、新增条目会真正进入包内，被移除的条目不出现。
   * 大图/图标键只在确有图时写——用户移除大图后不再产生悬空引用；
   * 引用基础原图时（复刻资产路径）按路径反推原资源 instance，拷贝条目
   * 与原条目共享同一份图。造价/预算编辑不在包内（GlassBox 模拟侧）→ warnings。 */
  function buildGameExport(): {
    bytes: Uint8Array;
    entryCount: number;
    localeCount: number;
    warnings: string[];
  } {
    const data = replica.value;
    const entries: MenuExportEntry[] = [];
    const locale: LocaleString[] = [];
    const warnings: string[] = [];
    const seen = new Set<string>();
    const effective = allEffectiveEntries();
    for (const { category, entries: menuEntries } of effective) {
      for (const { tool } of menuEntries) {
        // 最小变更集合：只有被编辑过、新增过、或被移出过菜单的条目才进包。
        // （同一 instance 挂多个菜单时只在首次出现处导出一条 property。）
        const edit = edits.value[tool.id];
        const isNew = (added.value[category.id] ?? []).some((t) => t.id === tool.id);
        const removedFrom = Object.keys(removed.value).filter((menuId) =>
          (removed.value[menuId] ?? []).includes(tool.id),
        );
        if (!edit && !isNew && removedFrom.length === 0) continue;
        if (seen.has(tool.id)) continue;
        seen.add(tool.id);
        const knownInstance = /^[0-9A-F]{8}$/.test(tool.id);
        const instance = knownInstance
          ? Number.parseInt(tool.id, 16)
          : fnv1Lower(`menu-item:${category.id}:${tool.id}:${tool.label}`);
        // 归属容器 = 有效归属：真实游戏 instance 在复刻数据中的全部分类，
        // 减去被移出；新增/拷贝条目落到其所在分类（必须是真实容器 id）。
        const resolvedCategories = knownInstance
          ? Object.keys(data?.tools ?? {})
              .filter((catId) =>
                (data?.tools[catId] ?? []).some((t) => t.instance === tool.id)
                && !(removed.value[catId] ?? []).includes(tool.id),
              )
              .map((catId) => Number.parseInt(catId, 16))
          : [];
        const ownContainer = !knownInstance && /^[0-9A-F]{8}$/.test(category.id)
          ? [Number.parseInt(category.id, 16)]
          : [];
        const uiCategories = [...new Set([...resolvedCategories, ...ownContainer])];
        if (uiCategories.length === 0) {
          warnings.push(
            knownInstance
              ? `${tool.label || tool.id}：已从所有菜单移除；覆盖包无法表达整删除，本次不写入`
              : `${tool.label || tool.id}：所在菜单「${category.label}」(${category.id}) 不是游戏容器 instance，无法编译`,
          );
          continue;
        }
        const titleStringId = fnv1Lower(`str:title:${tool.id}:${tool.label}`);
        locale.push({ table: LOCALE_TABLES.toolName, id: titleStringId, text: tool.label });
        let descStringId: number | undefined;
        let unlockStringId: number | undefined;
        if (tool.desc) {
          descStringId = fnv1Lower(`str:desc:${tool.id}:${tool.desc}`);
          locale.push({ table: LOCALE_TABLES.toolDesc, id: descStringId, text: tool.desc });
        }
        if (tool.unlock) {
          unlockStringId = fnv1Lower(`str:unlock:${tool.id}:${tool.unlock}`);
          locale.push({ table: LOCALE_TABLES.toolUnlock, id: unlockStringId, text: tool.unlock });
        }
        // 图片三态：dataURL = 新图（随包写入 + 新 instance）；资产路径 =
        // 引用基础原图（只写键，instance 从文件名反推）；null = 已移除（不写键）。
        const croppedIcon =
          typeof tool.preview === "string" && tool.preview.startsWith("data:") ? tool.preview : undefined;
        const croppedMarquee =
          typeof tool.marquee === "string" && tool.marquee.startsWith("data:") ? tool.marquee : undefined;
        const preservedIcon = typeof tool.preview === "string" && !tool.preview.startsWith("data:")
          ? iconInstanceFromAsset(tool.preview)
          : null;
        const preservedMarquee = typeof tool.marquee === "string" && !tool.marquee.startsWith("data:")
          ? marqueeInstanceFromAsset(tool.marquee)
          : null;
        const iconInstance = croppedIcon
          ? fnv1Lower(`icon:${tool.id}`)
          : preservedIcon ?? undefined;
        const marqueeInstance = croppedMarquee ? instance : preservedMarquee ?? undefined;
        if (edits.value[tool.id]?.cost != null || edits.value[tool.id]?.upkeep != null) {
          warnings.push(`${tool.label || tool.id}：造价/预算编辑在 GlassBox 模拟侧，属性覆盖不含`);
        }
        entries.push({
          instance,
          uiCategories,
          uiPosition: tool.pos,
          titleStringId,
          descStringId,
          unlockStringId,
          iconInstance,
          iconPng: croppedIcon ? dataUrlToBytes(croppedIcon) : undefined,
          marqueeInstance,
          marqueeJpg: croppedMarquee ? dataUrlToBytes(croppedMarquee) : undefined,
          locked: tool.locked ?? false,
        });
      }
    }
    // 从唯一菜单里被移除的条目不会出现在任何有效视图 → 主循环看不见。
    // 覆盖包无法表达「整删除」（不写 = 游戏保留原条目），显式告知而不是静默。
    for (const [menuId, ids] of Object.entries(removed.value)) {
      for (const id of ids) {
        if (seen.has(id)) continue;
        const label = categories.value.find((category) => category.id === menuId)?.label ?? menuId;
        warnings.push(`「${label}」中移除的 ${id}：覆盖包无法表达整删除，游戏内将保留原条目`);
      }
    }
    const result = buildMenuExport({ entries, locale });
    return { bytes: result.package, entryCount: entries.length, localeCount: locale.length, warnings };
  }

  /** 基础 → 当前覆盖 的菜单 schema 文本 diff（工具条 diff 预览用）。
   * 与 exportSchema/buildGameExport 同一有效模型，只列出有变化的分类与条目。 */
  function schemaDiffText(): string {
    const lines: string[] = [];
    for (const { category, entries } of allEffectiveEntries()) {
      const baseById = new Map(category.items.map((tool) => [tool.id, tool]));
      const catLines: string[] = [];
      for (const entry of entries) {
        const tool = entry.tool;
        const base = baseById.get(tool.id);
        if (!base) {
          catLines.push(`  + ${tool.label || tool.id} (${tool.id}) 新增`);
          continue;
        }
        const fields: string[] = [];
        const cmp = (name: string, a: unknown, b: unknown): void => {
          const sa = a == null || a === "" ? "—" : String(a);
          const sb = b == null || b === "" ? "—" : String(b);
          if (sa !== sb) fields.push(`${name}: ${sb} → ${sa}`);
        };
        cmp("名称", tool.label, base.label);
        cmp("排序", tool.pos, base.pos);
        cmp("造价", tool.cost ?? null, base.cost ?? null);
        cmp("维护", tool.upkeep ?? null, base.upkeep ?? null);
        if ((tool.desc ?? "") !== (base.desc ?? "")) fields.push("描述: 已修改");
        if ((tool.unlock ?? "") !== (base.unlock ?? "")) fields.push("解锁: 已修改");
        if ((tool.locked ?? false) !== (base.locked ?? false)) {
          fields.push(`锁定: ${(base.locked ?? false) ? "开" : "关"} → ${(tool.locked ?? false) ? "开" : "关"}`);
        }
        const image = (value: string | null | undefined): string =>
          value == null ? "已移除" : value.startsWith("data:") ? "新图" : value;
        if ((tool.preview ?? null) !== (base.preview ?? null)) {
          fields.push(`槽位图标: ${image(tool.preview)}`);
        }
        if ((tool.marquee ?? null) !== (base.marquee ?? null)) {
          fields.push(`大图: ${image(tool.marquee)}`);
        }
        if (JSON.stringify(tool.stats ?? []) !== JSON.stringify(base.stats ?? [])) {
          fields.push("统计行: 已修改");
        }
        if (fields.length > 0) {
          catLines.push(`  ~ ${base.label || tool.id} (${tool.id})`);
          for (const field of fields) catLines.push(`      ${field}`);
        }
      }
      for (const id of removed.value[category.id] ?? []) {
        const base = baseById.get(id);
        if (base && !entries.some((entry) => entry.tool.id === id)) {
          catLines.push(`  - ${base.label || id} (${id}) 移除`);
        }
      }
      if (catLines.length > 0) {
        lines.push(`[${category.label} ${category.id}]`);
        lines.push(...catLines);
      }
    }
    return lines.join("\n");
  }

  /** 单条菜单条目 → .property 资源字节（Sheet 内按条目导出）。 */
  function buildEntryProperty(itemId: string): Uint8Array | null {
    const entry = activeMenu.value?.entries.find((candidate) => candidate.tool.id === itemId);
    const data = replica.value;
    if (!entry || !data) return null;
    if (!/^[0-9A-F]{8}$/.test(entry.tool.id)) return null;
    const instance = Number.parseInt(entry.tool.id, 16);
    const exported: MenuExportEntry = {
      instance,
      uiCategories: [Number.parseInt(entry.menuId, 16)],
      uiPosition: entry.tool.pos,
      titleStringId: fnv1Lower(`str:title:${entry.tool.id}:${entry.tool.label}`),
      descStringId: entry.tool.desc ? fnv1Lower(`str:desc:${entry.tool.id}:${entry.tool.desc}`) : undefined,
      unlockStringId: entry.tool.unlock ? fnv1Lower(`str:unlock:${entry.tool.id}:${entry.tool.unlock}`) : undefined,
      iconInstance: undefined,
      locked: entry.tool.locked ?? false,
    };
    void data;
    return buildMenuPropertyResource(exported);
  }

  function resetAll(): void {
    edits.value = {};
    added.value = {};
    removed.value = {};
    resetEditor();
  }

  return {
    replica,
    loading,
    loadError,
    selectedMenuId,
    entered,
    edits,
    added,
    removed,
    editingEntry,
    editorStack,
    customCategories,
    viewMode,
    categories,
    activeMenu,
    hydrate,
    load,
    enterMenu,
    setViewMode,
    leaveMenu,
    openEditor,
    openSubmenuItem,
    closeEditor,
    resetEditor,
    addCategory,
    removeCategory,
    updateItem,
    addItem,
    addUniversitySidewalkToRoad,
    duplicateToMenu,
    membershipsOf,
    copyTargetMenus,
    removeItem,
    exportOverlay,
    exportSchema,
    schemaDiffText,
    buildGameExport,
    buildEntryProperty,
    resetAll,
  };
});
