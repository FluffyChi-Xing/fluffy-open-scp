/**
 * 文档编辑页签的纯状态逻辑（仿 Obsidian 多页签）。
 * 只做不可变的状态变换与持久化序列化；异步加载/保存由
 * use-document-tabs 组合式函数编排。
 */
import type { MarkdownDocument } from "@/api";

export interface EditorTab {
  /** 工作区相对路径，作为页签唯一键。 */
  path: string;
  /** 当前编辑草稿。 */
  draft: string;
  /** 后端乐观锁版本号。 */
  revision: string;
  mode: "edit" | "preview";
  dirty: boolean;
  loading: boolean;
  error: string;
}

export interface TabsState {
  tabs: EditorTab[];
  activePath: string;
}

export const EMPTY_TABS: TabsState = { tabs: [], activePath: "" };

export function tabTitle(path: string): string {
  return path.split("/").pop() ?? path;
}

export function findTab(
  tabs: readonly EditorTab[],
  path: string,
): EditorTab | undefined {
  return tabs.find((tab) => tab.path === path);
}

export function activeTab(state: TabsState): EditorTab | null {
  return findTab(state.tabs, state.activePath) ?? null;
}

function replaceTab(tabs: readonly EditorTab[], next: EditorTab): EditorTab[] {
  return tabs.map((tab) => (tab.path === next.path ? next : tab));
}

/** 打开（或激活）文档：已打开 → 仅激活；否则追加一个加载中的页签。 */
export function openTab(state: TabsState, path: string): TabsState {
  if (findTab(state.tabs, path)) return { ...state, activePath: path };
  return {
    tabs: [
      ...state.tabs,
      {
        path,
        draft: "",
        revision: "",
        mode: "edit",
        dirty: false,
        loading: true,
        error: "",
      },
    ],
    activePath: path,
  };
}

/** 文档加载完成：写入内容与版本号。 */
export function tabLoaded(
  state: TabsState,
  document: MarkdownDocument,
): TabsState {
  const tab = findTab(state.tabs, document.relativePath);
  if (!tab) return state;
  return {
    ...state,
    tabs: replaceTab(state.tabs, {
      ...tab,
      draft: document.content,
      revision: document.revision,
      loading: false,
      dirty: false,
      error: "",
    }),
  };
}

/** 文档加载失败：保留页签并展示错误（持久化恢复时的失败由调用方关闭）。 */
export function tabFailed(
  state: TabsState,
  path: string,
  error: string,
): TabsState {
  const tab = findTab(state.tabs, path);
  if (!tab) return state;
  return {
    ...state,
    tabs: replaceTab(state.tabs, { ...tab, loading: false, error }),
  };
}

/**
 * 关闭页签：激活邻居页签（优先右侧，其次左侧）。
 * 返回 null 表示该页签不存在（调用方据此区分「无变化」）。
 */
export function closeTab(state: TabsState, path: string): TabsState | null {
  const index = state.tabs.findIndex((tab) => tab.path === path);
  if (index === -1) return null;
  const tabs = state.tabs.filter((tab) => tab.path !== path);
  let activePath = state.activePath;
  if (state.activePath === path) {
    const neighbor = tabs[Math.min(index, tabs.length - 1)];
    activePath = neighbor?.path ?? "";
  }
  return { tabs, activePath };
}

/** 更新草稿（编辑输入）：标记脏状态。 */
export function updateDraft(
  state: TabsState,
  path: string,
  draft: string,
): TabsState {
  const tab = findTab(state.tabs, path);
  if (!tab || tab.draft === draft) return state;
  return {
    ...state,
    tabs: replaceTab(state.tabs, { ...tab, draft, dirty: true }),
  };
}

/** 切换编辑/预览模式。 */
export function setTabMode(
  state: TabsState,
  path: string,
  mode: EditorTab["mode"],
): TabsState {
  const tab = findTab(state.tabs, path);
  if (!tab) return state;
  return { ...state, tabs: replaceTab(state.tabs, { ...tab, mode }) };
}

/** 保存成功：同步版本号并清除脏标记。 */
export function tabSaved(
  state: TabsState,
  document: MarkdownDocument,
): TabsState {
  const tab = findTab(state.tabs, document.relativePath);
  if (!tab) return state;
  return {
    ...state,
    tabs: replaceTab(state.tabs, {
      ...tab,
      draft: document.content,
      revision: document.revision,
      dirty: false,
      error: "",
    }),
  };
}

/** 重命名/移动后的路径重定位：精确匹配或目录前缀替换。 */
export function rebasePath(path: string, from: string, to: string): string {
  if (path === from) return to;
  if (path.startsWith(`${from}/`)) return `${to}${path.slice(from.length)}`;
  return path;
}

/** 对整个页签状态做路径重定位（重命名/移动条目后调用）。 */
export function rebaseTabs(
  state: TabsState,
  from: string,
  to: string,
): TabsState {
  const map = (path: string) => rebasePath(path, from, to);
  if (
    map(state.activePath) === state.activePath &&
    !state.tabs.some((tab) => map(tab.path) !== tab.path)
  ) {
    return state;
  }
  return {
    tabs: state.tabs.map((tab) => ({ ...tab, path: map(tab.path) })),
    activePath: map(state.activePath),
  };
}

/* ---- 持久化：只存路径与激活项，内容重新加载 ---- */

interface PersistedTabs {
  paths: string[];
  activePath: string;
}

export function serializeTabs(state: TabsState): string {
  const payload: PersistedTabs = {
    paths: state.tabs.map((tab) => tab.path),
    activePath: state.activePath,
  };
  return JSON.stringify(payload);
}

/** 解析持久化数据；格式不符或为空时返回 null（调用方放弃恢复）。 */
export function parseTabs(raw: string | null): PersistedTabs | null {
  if (!raw) return null;
  try {
    const value = JSON.parse(raw) as Partial<PersistedTabs>;
    if (!Array.isArray(value.paths)) return null;
    const paths = value.paths.filter(
      (path): path is string => typeof path === "string" && path.length > 0,
    );
    const activePath =
      typeof value.activePath === "string" && paths.includes(value.activePath)
        ? value.activePath
        : (paths[0] ?? "");
    return { paths, activePath };
  } catch {
    return null;
  }
}
