import { computed, shallowRef } from "vue";
import {
  tauriApi,
  type MarkdownDocument,
  type WorkspaceEntry,
  type WorkspaceStatus,
} from "@/api";

interface WorkspaceBackend {
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

export function useWorkspace() {
  const backend: WorkspaceBackend =
    typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
      ? tauriApi.workspace
      : browserBackend();
  const status = shallowRef<WorkspaceStatus | null>(null);
  const entries = shallowRef<WorkspaceEntry[]>([]);
  const selectedPath = shallowRef("");
  const document = shallowRef<MarkdownDocument | null>(null);
  const loading = shallowRef(false);
  const saving = shallowRef(false);
  const error = shallowRef("");
  const isConfigured = computed(
    () => status.value?.configured === true && status.value.available,
  );

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
  async function select(path: string) {
    selectedPath.value = path;
    document.value = null;
    error.value = "";
    try {
      document.value = await backend.readMarkdown(path);
    } catch (cause) {
      error.value = messageOf(cause);
    }
  }
  async function setRoot(path: string) {
    status.value = await backend.setRoot(path);
    entries.value = [];
    document.value = null;
    selectedPath.value = "";
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
  async function save(content: string) {
    if (!document.value) return;
    saving.value = true;
    error.value = "";
    try {
      document.value = await backend.writeMarkdown(
        document.value.relativePath,
        content,
        document.value.revision,
      );
    } catch (cause) {
      error.value = messageOf(cause);
    } finally {
      saving.value = false;
    }
  }
  return {
    status,
    entries,
    selectedPath,
    document,
    loading,
    saving,
    error,
    isConfigured,
    loadStatus,
    loadEntries,
    select,
    setRoot,
    createFolderIn,
    createMarkdownIn,
    renameEntry,
    moveEntry,
    save,
  };
}

function messageOf(cause: unknown) {
  if (cause && typeof cause === "object" && "message" in cause)
    return String(cause.message);
  return cause instanceof Error ? cause.message : String(cause);
}
