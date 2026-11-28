import { i18n } from "@/locales";
import {
  inject,
  onBeforeUnmount,
  onMounted,
  reactive,
  type InjectionKey,
} from "vue";
import { isTauri, tauriApi } from "@/api";
import { command } from "@/api/tauri";

export type EditorPackageKind = "assets" | "terrain";
export interface EditorPackage {
  packageId: number;
  path: string;
}
export interface EditorPackageScope {
  opened: { package: EditorPackage }[];
  directory: string;
  loading: boolean;
  missing: string[];
  failed: boolean;
  ready: Promise<void>;
  load(directory?: string): Promise<void>;
  openFile(path: string): Promise<{ package: EditorPackage }>;
}
export const EDITOR_PACKAGES: InjectionKey<EditorPackageScope> =
  Symbol("editorPackages");

/** Each mounted editor owns backend aliases, never explorer handles. */
export function useEditorPackages(kind: EditorPackageKind): EditorPackageScope {
  let generation = 0,
    disposed = false;
  const files = new Map<string, Promise<{ package: EditorPackage }>>();
  const fileIds = new Set<number>();
  let finishReady!: () => void;
  const ready = new Promise<void>((resolve) => {
    finishReady = resolve;
  });
  const release = async (ids: number[]) => {
    if (ids.length)
      await command("release_editor_packages", { packageIds: ids }).catch(
        () => {},
      );
  };
  const scope: EditorPackageScope = reactive({
    opened: [],
    directory: "",
    loading: true,
    missing: [],
    failed: false,
    ready,
    openFile(path: string) {
      let pending = files.get(path);
      if (!pending) {
        pending = command<EditorPackage>("acquire_editor_file", { path })
          .then(async (entry) => {
            if (disposed) {
              await release([entry.packageId]);
              throw new Error(i18n.global.t("editorPackages.closed"));
            }
            fileIds.add(entry.packageId);
            return { package: entry };
          })
          .catch((error) => {
            files.delete(path);
            throw error;
          });
        files.set(path, pending);
      }
      return pending;
    },
    async load(directory?: string) {
      const token = ++generation;
      scope.loading = true;
      scope.failed = false;
      const old = scope.opened.map((entry) => entry.package.packageId);
      scope.opened = [];
      scope.missing = [];
      await release(old);
      try {
        const configured =
          directory ?? (await tauriApi.settings.get()).gameDataPath ?? "";
        if (disposed || token !== generation) return;
        scope.directory = configured;
        if (!configured || !isTauri()) return;
        const result = await command<{
          packages: EditorPackage[];
          missing: string[];
        }>("acquire_editor_packages", {
          request: { directory: configured, kind },
        });
        if (disposed || token !== generation) {
          await release(result.packages.map((p) => p.packageId));
          return;
        }
        scope.opened = result.packages.map((p) => ({ package: p }));
        scope.missing = result.missing;
      } catch {
        if (!disposed && token === generation) scope.failed = true;
      } finally {
        if (token === generation) scope.loading = false;
      }
    },
  });
  onMounted(() => {
    void scope.load().finally(finishReady);
  });
  onBeforeUnmount(() => {
    disposed = true;
    generation++;
    const ids = [
      ...scope.opened.map((entry) => entry.package.packageId),
      ...fileIds,
    ];
    fileIds.clear();
    files.clear();
    scope.opened = [];
    void release(ids);
    finishReady();
  });
  return scope;
}

/** Embedded libraries consume the editor's scope, not the explorer store. */
export function useEditorPackageContext(): EditorPackageScope {
  return (
    inject(EDITOR_PACKAGES, null) ??
    reactive({
      opened: [],
      directory: "",
      loading: false,
      missing: [],
      failed: false,
      ready: Promise.resolve(),
      load: async () => {},
      openFile: async () => {
        throw new Error(i18n.global.t("editorPackages.failed"));
      },
    })
  );
}
