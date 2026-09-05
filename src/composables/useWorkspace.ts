import { computed, shallowRef } from "vue";
import {
  tauriApi,
  type MarkdownDocument,
  type WorkspaceFolder,
  type WorkspaceStatus,
} from "@/api";

interface WorkspaceBackend {
  get(): Promise<WorkspaceStatus>;
  list(): Promise<WorkspaceFolder[]>;
  createFolder(relativePath: string): Promise<WorkspaceFolder[]>;
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
  rename(relativePath: string, newName: string): Promise<WorkspaceFolder[]>;
  move(
    relativePath: string,
    targetDirectory: string,
  ): Promise<WorkspaceFolder[]>;
  setRoot(path: string): Promise<WorkspaceStatus>;
}

function browserBackend(): WorkspaceBackend {
  const status: WorkspaceStatus = {
    configured: true,
    rootPath: "D:/demo/workspace",
    available: true,
  };
  let folders: WorkspaceFolder[] = [
    {
      relativePath: "mods/example",
      readmeRelativePath: "mods/example/README.md",
    },
    {
      relativePath: "mods/example/nested",
      readmeRelativePath: "mods/example/nested/README.md",
    },
    {
      relativePath: "assets/arcology",
      readmeRelativePath: "assets/arcology/README.md",
    },
  ];
  const documents = new Map<string, string>();
  const sorted = () =>
    [...folders].sort((a, b) => a.relativePath.localeCompare(b.relativePath));
  const write = (relativePath: string, content: string): MarkdownDocument => {
    documents.set(relativePath, content);
    return {
      relativePath,
      content,
      size: content.length,
      revision: `mock-${content.length}-${documents.size}`,
    };
  };
  const rebase = (from: string, to: string) => {
    folders = folders.map((folder) => ({
      relativePath:
        folder.relativePath === from
          ? to
          : folder.relativePath.startsWith(`${from}/`)
            ? `${to}${folder.relativePath.slice(from.length)}`
            : folder.relativePath,
      readmeRelativePath: folder.readmeRelativePath
        ? folder.readmeRelativePath === `${from}/README.md`
          ? `${to}/README.md`
          : folder.readmeRelativePath.startsWith(`${from}/`)
            ? `${to}${folder.readmeRelativePath.slice(from.length)}`
            : folder.readmeRelativePath
        : undefined,
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
      folders = [
        ...folders,
        { relativePath, readmeRelativePath: `${relativePath}/README.md` },
      ];
      write(`${relativePath}/README.md`, "");
      return sorted();
    },
    async createMarkdown(relativePath, content) {
      write(relativePath, content);
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
  const folders = shallowRef<WorkspaceFolder[]>([]);
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
  async function loadFolders() {
    loading.value = true;
    error.value = "";
    try {
      folders.value = await backend.list();
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
    folders.value = [];
    document.value = null;
    selectedPath.value = "";
  }
  async function createFolder(path: string) {
    folders.value = await backend.createFolder(path);
  }
  async function createFolderIn(parent: string, name: string) {
    folders.value = await backend.createFolder(
      parent ? `${parent}/${name}` : name,
    );
  }
  async function createMarkdownIn(parent: string, name: string) {
    await backend.createMarkdown(parent ? `${parent}/${name}` : name, "");
    folders.value = await backend.list();
  }
  async function renameEntry(path: string, newName: string) {
    folders.value = await backend.rename(path, newName);
  }
  async function moveEntry(path: string, targetDirectory: string) {
    folders.value = await backend.move(path, targetDirectory);
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
    folders,
    selectedPath,
    document,
    loading,
    saving,
    error,
    isConfigured,
    loadStatus,
    loadFolders,
    select,
    setRoot,
    createFolder,
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
