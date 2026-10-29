/**
 * 文档编辑页签组合式函数：把 editor-tabs.ts 的纯状态变换与后端
 * 读写编排起来，并把打开的页签持久化到 localStorage（只存路径，
 * 内容在挂载恢复时重新加载）。
 */
import { computed, onMounted, shallowRef, watch } from "vue";
import {
  type EditorTab,
  type TabsState,
  closeTab as closeTabState,
  openTab,
  parseTabs,
  serializeTabs,
  setTabMode,
  tabFailed,
  tabLoaded,
  tabSaved,
  updateDraft,
  rebaseTabs,
  activeTab,
  findTab,
} from "./editor-tabs";
import { messageOf, resolveWorkspaceBackend } from "@/composables/useWorkspace";

const STORAGE_KEY = "openscp.workspace.editor-tabs";

export interface DocumentTabsOptions {
  /** 关闭有未保存修改的页签前询问；返回 false 取消关闭。 */
  confirmCloseDirty?: () => boolean;
}

export function useDocumentTabs(options: DocumentTabsOptions = {}) {
  const backend = resolveWorkspaceBackend();
  const state = shallowRef<TabsState>({ tabs: [], activePath: "" });
  /** 正在保存的页签路径（空串表示空闲）。 */
  const savingPath = shallowRef("");
  const active = computed<EditorTab | null>(() => activeTab(state.value));

  function commit(next: TabsState) {
    state.value = next;
  }

  async function loadInto(path: string) {
    try {
      const document = await backend.readMarkdown(path);
      commit(tabLoaded(state.value, document));
    } catch (cause) {
      commit(tabFailed(state.value, path, messageOf(cause)));
    }
  }

  /** 打开文档：已打开则仅激活，否则新建页签并加载内容。 */
  function open(path: string) {
    commit(openTab(state.value, path));
    void loadInto(path);
  }

  function close(path: string) {
    const tab = findTab(state.value.tabs, path);
    if (!tab) return;
    if (
      tab.dirty &&
      options.confirmCloseDirty &&
      !options.confirmCloseDirty()
    ) {
      return;
    }
    const next = closeTabState(state.value, path);
    if (next) commit(next);
  }

  function activate(path: string) {
    commit({ ...state.value, activePath: path });
  }

  function setMode(path: string, mode: EditorTab["mode"]) {
    commit(setTabMode(state.value, path, mode));
  }

  function setDraft(path: string, draft: string) {
    commit(updateDraft(state.value, path, draft));
  }

  async function save(path = state.value.activePath) {
    const tab = findTab(state.value.tabs, path);
    if (!tab || tab.loading || savingPath.value) return;
    savingPath.value = path;
    try {
      const document = await backend.writeMarkdown(
        path,
        tab.draft,
        tab.revision || undefined,
      );
      commit(tabSaved(state.value, document));
    } catch (cause) {
      commit(tabFailed(state.value, path, messageOf(cause)));
    } finally {
      savingPath.value = "";
    }
  }

  /** 重命名/移动条目后重定位打开中的页签路径。 */
  function rebase(from: string, to: string) {
    commit(rebaseTabs(state.value, from, to));
  }

  watch(
    state,
    (value) => {
      try {
        localStorage.setItem(STORAGE_KEY, serializeTabs(value));
      } catch {
        /* 存储不可用时忽略（隐私模式等） */
      }
    },
    { deep: true },
  );

  onMounted(async () => {
    let persisted: ReturnType<typeof parseTabs>;
    try {
      persisted = parseTabs(localStorage.getItem(STORAGE_KEY));
    } catch {
      persisted = null;
    }
    if (!persisted?.paths.length) return;
    let next = state.value;
    for (const path of persisted.paths) next = openTab(next, path);
    commit({ ...next, activePath: persisted.activePath });
    await Promise.all(persisted.paths.map((path) => loadInto(path)));
    // 恢复时已不存在的文档静默关闭（未编辑过，无丢失风险）
    for (const tab of state.value.tabs) {
      if (tab.error && !tab.dirty) {
        const nextTabs = closeTabState(state.value, tab.path);
        if (nextTabs) state.value = nextTabs;
      }
    }
  });

  return {
    state,
    active,
    savingPath,
    open,
    close,
    activate,
    setMode,
    setDraft,
    save,
    rebase,
  };
}
