import { computed, shallowRef } from "vue";
import { defineStore } from "pinia";
import { createDataSource, localDemoConfig } from "@/api/data-source";
import { isTauri, tauriApi } from "@/api";
import type {
  GameFolder,
  OpenPackageResponse,
  PackageFile,
  ResourcePage,
  ResourcePreview,
  ResourceSummary,
  Tgi,
} from "@/api/tauri";
import { useToast } from "@/composables/useToast";

const PAGE_SIZE = 100;

/**
 * 资源浏览器的全局状态：打开的 package、当前页、筛选与预览。
 * 放在 Pinia 里是为了在页面切换后保留（打开的 tab 记录不丢失），
 * 生命周期与 App 相同；package 句柄在关闭时显式释放。
 */
export const useGamePackagesStore = defineStore("gamePackages", () => {
  const source = createDataSource();
  const toast = useToast();
  const root = shallowRef("D:/ea-games/SimCity");
  const folders = shallowRef<GameFolder[]>([]);
  const files = shallowRef<PackageFile[]>([]);
  const opened = shallowRef<OpenPackageResponse[]>([]);
  const activePackageId = shallowRef<number | null>(null);
  const activePage = shallowRef<ResourcePage | null>(null);
  const typeFilter = shallowRef<number | null>(null);
  const selected = shallowRef<ResourceSummary | null>(null);
  const names = shallowRef<Record<string, string>>({});
  const preview = shallowRef<ResourcePreview | null>(null);
  const previewLoading = shallowRef(false);
  const previewError = shallowRef("");
  const loadingFolders = shallowRef(false);
  const loadingFiles = shallowRef(false);
  const loadingPackage = shallowRef(false);
  const filter = shallowRef("");
  const selectedFolder = shallowRef("");
  let previewRequest = 0;
  const demo = shallowRef(
    localDemoConfig() !== null || !("__TAURI_INTERNALS__" in window),
  );
  const activePackage = computed(
    () =>
      opened.value.find(
        (item) => item.package.packageId === activePackageId.value,
      ) ?? null,
  );
  const category = computed(() =>
    typeFilter.value === null ? "all" : String(typeFilter.value),
  );
  const currentPage = computed(
    () => Math.floor((activePage.value?.offset ?? 0) / PAGE_SIZE) + 1,
  );
  const totalPages = computed(() => {
    const page = activePage.value;
    if (!page || page.total === 0) return 1;
    return Math.max(1, Math.ceil(page.total / (page.limit || PAGE_SIZE)));
  });
  const canPrev = computed(() => (activePage.value?.offset ?? 0) > 0);
  const canNext = computed(() => currentPage.value < totalPages.value);

  async function loadFolders(path = root.value) {
    loadingFolders.value = true;
    try {
      folders.value = await source.listGameTree(path);
    } catch {
      toast.error("无法读取游戏目录");
    } finally {
      loadingFolders.value = false;
    }
  }
  /** 首次进入资源页时按持久化设置初始化目录树（已加载过则跳过）。 */
  async function initFromSettings() {
    if (folders.value.length || loadingFolders.value) return;
    try {
      if (!isTauri()) return;
      const settings = await tauriApi.settings.get();
      const path = settings.gameDataPath;
      if (path) {
        root.value = path;
        await loadFolders(path);
      }
    } catch {
      // 设置读取失败时保持硬编码默认，用户仍可手动导入
    }
  }
  async function selectFolder(path: string) {
    selectedFolder.value = path;
    loadingFiles.value = true;
    try {
      files.value = await source.listPackageFiles(path);
    } catch {
      files.value = [];
      toast.error("无法读取文件夹内容");
    } finally {
      loadingFiles.value = false;
    }
  }
  async function openFile(file: PackageFile) {
    loadingPackage.value = true;
    try {
      const result = await source.openPackage(file.path);
      const existing = opened.value.find(
        (item) => item.package.path === file.path,
      );
      if (existing) {
        activePackageId.value = existing.package.packageId;
        activePage.value = existing.resources;
      } else {
        opened.value = [...opened.value, result];
        activePackageId.value = result.package.packageId;
        activePage.value = result.resources;
      }
      typeFilter.value = null;
      selected.value = null;
      void loadNames(activePackageId.value, activePage.value);
    } catch {
      toast.error("Package 无法解析");
    } finally {
      loadingPackage.value = false;
    }
  }
  /** TGI 搜索：更新过滤词并回到第一页（空串清除）。 */
  async function applyFilter(value: string) {
    if (filter.value === value) return;
    filter.value = value;
    if (activePackage.value) await loadPage(0);
  }

  async function loadPage(offset = 0) {
    if (!activePackage.value) return;
    loadingPackage.value = true;
    try {
      activePage.value = await source.listResources(
        activePackage.value.package.packageId,
        offset,
        PAGE_SIZE,
        filter.value || undefined,
        typeFilter.value ?? undefined,
      );
      void loadNames(activePackage.value.package.packageId, activePage.value);
    } catch {
      toast.error("资源列表加载失败");
    } finally {
      loadingPackage.value = false;
    }
  }
  async function chooseCategory(key: string) {
    typeFilter.value = key === "all" ? null : Number(key);
    await loadPage(0);
  }
  async function prevPage() {
    const page = activePage.value;
    if (page && canPrev.value)
      await loadPage(Math.max(0, page.offset - page.limit));
  }
  async function nextPage() {
    const page = activePage.value;
    if (page && canNext.value) await loadPage(page.offset + page.limit);
  }
  function tgiKey(tgi: Tgi) {
    return `${tgi.typeId}:${tgi.group}:${tgi.instance}`;
  }
  async function loadNames(
    packageId: number,
    page: ResourcePage | null,
  ): Promise<void> {
    if (!page) return;
    const missing = page.items.filter((item) => !names.value[tgiKey(item.tgi)]);
    if (!missing.length) return;
    try {
      const resolved = await source.resolveNames(
        packageId,
        missing.map((item) => item.tgi),
      );
      const next = { ...names.value };
      for (const entry of resolved) {
        if (entry.displayName) next[tgiKey(entry.tgi)] = entry.displayName;
      }
      names.value = next;
    } catch {
      // 名称解析失败不影响资源表，保留 TGI 回退显示
    }
  }
  function releasePreviewUrl() {
    if (
      preview.value?.kind === "image" &&
      preview.value.src.startsWith("blob:")
    ) {
      URL.revokeObjectURL(preview.value.src);
    }
  }
  async function selectResource(resource: ResourceSummary) {
    selected.value = resource;
    releasePreviewUrl();
    preview.value = null;
    previewError.value = "";
    if (!activePackage.value) return;
    const request = ++previewRequest;
    previewLoading.value = true;
    try {
      const result = await source.previewResource(
        activePackage.value.package.packageId,
        resource,
      );
      if (request === previewRequest) preview.value = result;
      else if (result.kind === "image" && result.src.startsWith("blob:"))
        URL.revokeObjectURL(result.src);
    } catch {
      if (request === previewRequest) previewError.value = "资源预览失败";
      toast.error("资源读取失败");
    } finally {
      if (request === previewRequest) previewLoading.value = false;
    }
  }
  async function closePackage(id: number) {
    try {
      await source.closePackage(id);
      opened.value = opened.value.filter(
        (item) => item.package.packageId !== id,
      );
      if (activePackageId.value === id) {
        const next = opened.value.at(-1);
        activePackageId.value = next?.package.packageId ?? null;
        activePage.value = next?.resources ?? null;
        typeFilter.value = null;
        selected.value = null;
        releasePreviewUrl();
        preview.value = null;
      }
    } catch {
      toast.error("Package 关闭失败");
    }
  }
  function choosePackage(id: number) {
    const item = opened.value.find((entry) => entry.package.packageId === id);
    if (item) {
      activePackageId.value = id;
      activePage.value = item.resources;
      typeFilter.value = null;
      selected.value = null;
      releasePreviewUrl();
      preview.value = null;
    }
  }
  function tgiLabel(resource: ResourceSummary) {
    return [resource.tgi.typeId, resource.tgi.group, resource.tgi.instance]
      .map((value) => value.toString(16).padStart(8, "0"))
      .join(":");
  }
  function resourceLabel(resource: ResourceSummary) {
    return (
      names.value[tgiKey(resource.tgi)] ??
      `0x${resource.tgi.instance.toString(16).padStart(8, "0")}`
    );
  }
  function typeNameOf(resource: ResourceSummary) {
    const found = activePage.value?.typeCounts.find(
      (entry) => entry.typeId === resource.tgi.typeId,
    );
    return (
      found?.name ?? resource.tgi.typeId.toString(16).padStart(8, "0").toUpperCase()
    );
  }

  return {
    root,
    folders,
    files,
    opened,
    activePackageId,
    activePackage,
    activePage,
    category,
    currentPage,
    totalPages,
    canPrev,
    canNext,
    selected,
    resourceLabel,
    typeNameOf,
    preview,
    previewLoading,
    previewError,
    loadingFolders,
    loadingFiles,
    loadingPackage,
    filter,
    applyFilter,
    selectedFolder,
    demo,
    loadFolders,
    initFromSettings,
    selectFolder,
    openFile,
    loadPage,
    prevPage,
    nextPage,
    chooseCategory,
    selectResource,
    closePackage,
    choosePackage,
    tgiLabel,
  };
});
