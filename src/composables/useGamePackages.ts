import { computed, shallowRef } from 'vue'
import { createDataSource, localDemoConfig } from '@/api/data-source'
import type { GameFolder, OpenPackageResponse, PackageFile, ResourcePage, ResourceSummary } from '@/api/tauri'
import { useToast } from './useToast'

export function useGamePackages() {
  const source = createDataSource()
  const toast = useToast()
  const root = shallowRef('D:/ea-games/SimCity')
  const folders = shallowRef<GameFolder[]>([])
  const files = shallowRef<PackageFile[]>([])
  const opened = shallowRef<OpenPackageResponse[]>([])
  const activePackageId = shallowRef<number | null>(null)
  const activePage = shallowRef<ResourcePage | null>(null)
  const selected = shallowRef<ResourceSummary | null>(null)
  const loadingFolders = shallowRef(false)
  const loadingFiles = shallowRef(false)
  const loadingPackage = shallowRef(false)
  const filter = shallowRef('')
  const selectedFolder = shallowRef('')
  const demo = shallowRef(localDemoConfig() !== null || !('__TAURI_INTERNALS__' in window))
  const activePackage = computed(() => opened.value.find((item) => item.package.packageId === activePackageId.value) ?? null)

  async function loadFolders(path = root.value) {
    loadingFolders.value = true
    try { folders.value = await source.listGameTree(path) } catch { toast.error('无法读取游戏目录') } finally { loadingFolders.value = false }
  }
  async function selectFolder(path: string) {
    selectedFolder.value = path
    loadingFiles.value = true
    try { files.value = await source.listPackageFiles(path) } catch { files.value = []; toast.error('无法读取文件夹内容') } finally { loadingFiles.value = false }
  }
  async function openFile(file: PackageFile) {
    loadingPackage.value = true
    try {
      const result = await source.openPackage(file.path)
      const existing = opened.value.find((item) => item.package.path === file.path)
      if (existing) activePackageId.value = existing.package.packageId
      else { opened.value = [...opened.value, result]; activePackageId.value = result.package.packageId }
      activePage.value = existing?.resources ?? result.resources
      selected.value = null
    } catch { toast.error('Package 无法解析') } finally { loadingPackage.value = false }
  }
  async function loadPage(offset = 0) {
    if (!activePackage.value) return
    loadingPackage.value = true
    try { activePage.value = await source.listResources(activePackage.value.package.packageId, offset, 100, filter.value || undefined) } catch { toast.error('资源列表加载失败') } finally { loadingPackage.value = false }
  }
  async function selectResource(resource: ResourceSummary) {
    selected.value = resource
    if (!activePackage.value) return
    try { await source.readResourceBytes(activePackage.value.package.packageId, resource.tgi, 0, Math.min(4096, resource.decompressedSize)) } catch { toast.error('资源读取失败') }
  }
  function closePackage(id: number) { opened.value = opened.value.filter((item) => item.package.packageId !== id); if (activePackageId.value === id) { const next = opened.value.at(-1); activePackageId.value = next?.package.packageId ?? null; activePage.value = next?.resources ?? null; selected.value = null } }
  function choosePackage(id: number) { const item = opened.value.find((entry) => entry.package.packageId === id); if (item) { activePackageId.value = id; activePage.value = item.resources } }
  function tgiLabel(resource: ResourceSummary) { return [resource.tgi.typeId, resource.tgi.group, resource.tgi.instance].map((value) => value.toString(16).padStart(8, '0')).join(':') }

  return { root, folders, files, opened, activePackageId, activePackage, activePage, selected, loadingFolders, loadingFiles, loadingPackage, filter, selectedFolder, demo, loadFolders, selectFolder, openFile, loadPage, selectResource, closePackage, choosePackage, tgiLabel }
}
