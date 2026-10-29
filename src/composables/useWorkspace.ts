import { computed, shallowRef } from "vue";
import {
  tauriApi,
  type LegacySyncOutcome,
  type LegacySyncStatus,
  type MarkdownDocument,
  type WorkspaceEntry,
  type WorkspaceStatus,
} from "@/api";

export interface WorkspaceBackend {
  get(): Promise<WorkspaceStatus>;
  list(): Promise<WorkspaceEntry[]>;
  createFolder(relativePath: string): Promise<WorkspaceEntry[]>;
  createMarkdown(
    relativePath: string,
    content: string,
  ): Promise<MarkdownDocument>;
  readMarkdown(relativePath: string): Promise<MarkdownDocument>;
  writeMarkdown(
    relativePath: string,
    content: string,
    expectedRevision?: string,
  ): Promise<MarkdownDocument>;
  rename(relativePath: string, newName: string): Promise<WorkspaceEntry[]>;
  move(
    relativePath: string,
    targetDirectory: string,
  ): Promise<WorkspaceEntry[]>;
  setRoot(path: string): Promise<WorkspaceStatus>;
  /** 历史目录迁移状态（2026-10-29 目录治理）：legacyDir ≠ internalDir
   * 且有待同步条目时 canSync=true。 */
  legacyStatus(): Promise<LegacySyncStatus>;
  /** 旧目录内容合并拷入内部目录并切换配置（不可逆的一次性迁移）。 */
  syncLegacy(): Promise<LegacySyncOutcome>;
}

function browserBackend(): WorkspaceBackend {
  const status: WorkspaceStatus = {
    configured: true,
    rootPath: "D:/demo/workspace",
    available: true,
  };
  let entries: WorkspaceEntry[] = [
    { relativePath: "mods", kind: "folder" },
    { relativePath: "mods/example", kind: "folder" },
    { relativePath: "mods/example/README.md", kind: "file" },
    { relativePath: "assets", kind: "folder" },
    { relativePath: "assets/arcology", kind: "folder" },
    { relativePath: "assets/arcology/README.md", kind: "file" },
  ];
  const documents = new Map<string, string>();
  const sorted = () =>
    [...entries].sort((a, b) => a.relativePath.localeCompare(b.relativePath));
  const write = (relativePath: string, content: string): MarkdownDocument => {
    documents.set(relativePath, content);
    return {
      relativePath,
      content,
      size: content.length,
      revision: `mock-${content.length}-${documents.size}`,
    };
  };
  const upsertFile = (relativePath: string) => {
    if (!entries.some((entry) => entry.relativePath === relativePath)) {
      entries = [...entries, { relativePath, kind: "file" }];
    }
  };
  const rebase = (from: string, to: string) => {
    entries = entries.map((entry) => ({
      kind: entry.kind,
      relativePath:
        entry.relativePath === from
          ? to
          : entry.relativePath.startsWith(`${from}/`)
            ? `${to}${entry.relativePath.slice(from.length)}`
            : entry.relativePath,
    }));
  };
  return {
    async get() {
      return status;
    },
    async list() {
      return sorted();
    },
    async setRoot() {
      return status;
    },
    async legacyStatus() {
      return {
        internalDir: "D:/demo/workspace",
        legacyDir: null,
        canSync: false,
        pendingCount: 0,
      };
    },
    async syncLegacy() {
      return {
        copiedCount: 0,
        status: {
          internalDir: "D:/demo/workspace",
          legacyDir: null,
          canSync: false,
          pendingCount: 0,
        },
      };
    },
    async createFolder(relativePath) {
      if (!entries.some((entry) => entry.relativePath === relativePath)) {
        entries = [...entries, { relativePath, kind: "folder" }];
      }
      return sorted();
    },
    async createMarkdown(relativePath, content) {
      write(relativePath, content);
      upsertFile(relativePath);
      return {
        relativePath,
        content,
        size: content.length,
        revision: "mock-new",
      };
    },
    async readMarkdown(relativePath) {
      return {
        relativePath,
        content: documents.get(relativePath) ?? `# ${relativePath}\n`,
        size: 0,
        revision: "mock-read",
      };
    },
    async writeMarkdown(relativePath, content) {
      return write(relativePath, content);
    },
    async rename(relativePath, newName) {
      const parent = relativePath.includes("/")
        ? relativePath.slice(0, relativePath.lastIndexOf("/"))
        : "";
      rebase(relativePath, parent ? `${parent}/${newName}` : newName);
      return sorted();
    },
    async move(relativePath, targetDirectory) {
      const name = relativePath.split("/").pop() ?? relativePath;
      rebase(
        relativePath,
        targetDirectory ? `${targetDirectory}/${name}` : name,
      );
      return sorted();
    },
  };
}

/** 桌面端走 Tauri 文件能力，浏览器预览走内存 mock。 */
export function resolveWorkspaceBackend(): WorkspaceBackend {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? tauriApi.workspace
    : browserBackend();
}

export function useWorkspace() {
  const backend = resolveWorkspaceBackend();
  const status = shallowRef<WorkspaceStatus | null>(null);
  const entries = shallowRef<WorkspaceEntry[]>([]);
  const loading = shallowRef(false);
  const error = shallowRef("");
  const isConfigured = computed(
    () => status.value?.configured === true && status.value.available,
  );
  /** 历史目录迁移状态：legacyDir 存在时页面显示同步按钮，canSync 置灰。 */
  const legacyStatus = shallowRef<LegacySyncStatus | null>(null);
  const syncing = shallowRef(false);

  async function loadLegacyStatus() {
    try {
      legacyStatus.value = await backend.legacyStatus();
    } catch {
      legacyStatus.value = null;
    }
  }
  /** 一次性迁移：拷入内部目录后切换配置，返回拷贝条目数。 */
  async function syncLegacy(): Promise<number> {
    syncing.value = true;
    error.value = "";
    try {
      const outcome = await backend.syncLegacy();
      legacyStatus.value = outcome.status;
      entries.value = await backend.list();
      return outcome.copiedCount;
    } catch (cause) {
      error.value = messageOf(cause);
      return 0;
    } finally {
      syncing.value = false;
    }
  }

  async function loadStatus() {
    loading.value = true;
    error.value = "";
    try {
      status.value = await backend.get();
    } catch (cause) {
      error.value = messageOf(cause);
    } finally {
      loading.value = false;
    }
  }
  async function loadEntries() {
    loading.value = true;
    error.value = "";
    try {
      entries.value = await backend.list();
    } catch (cause) {
      error.value = messageOf(cause);
    } finally {
      loading.value = false;
    }
  }
  async function createFolderIn(parent: string, name: string) {
    entries.value = await backend.createFolder(
      parent ? `${parent}/${name}` : name,
    );
  }
  async function createMarkdownIn(parent: string, name: string) {
    await backend.createMarkdown(parent ? `${parent}/${name}` : name, "");
    entries.value = await backend.list();
  }
  async function renameEntry(path: string, newName: string) {
    entries.value = await backend.rename(path, newName);
  }
  async function moveEntry(path: string, targetDirectory: string) {
    entries.value = await backend.move(path, targetDirectory);
  }
  return {
    status,
    entries,
    loading,
    error,
    isConfigured,
    legacyStatus,
    syncing,
    loadStatus,
    loadEntries,
    loadLegacyStatus,
    syncLegacy,
    createFolderIn,
    createMarkdownIn,
    renameEntry,
    moveEntry,
  };
}

export function messageOf(cause: unknown) {
  if (cause && typeof cause === "object" && "message" in cause)
    return String(cause.message);
  return cause instanceof Error ? cause.message : String(cause);
}
