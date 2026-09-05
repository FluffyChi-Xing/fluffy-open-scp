import { isTauri, tauriApi } from './index'
import type { GameFolder, OpenPackageResponse, PackageFile, PackageHistory, ResourceBytes, ResourcePage, Tgi, WorkspaceFolder, WorkspaceStatus } from './tauri'
import { mockOverview } from './mock-data'

export interface LocalDemoConfig { version: 1; mode: 'mock' }
export interface OpenScpDataSource {
  overview(): Promise<typeof mockOverview>
  listGameTree(root: string): Promise<GameFolder[]>
  listPackageFiles(folder: string): Promise<PackageFile[]>
  openPackage(path: string): Promise<OpenPackageResponse>
  listResources(packageId: number, offset: number, limit: number, filter?: string): Promise<ResourcePage>
  readResourceBytes(packageId: number, tgi: Tgi, offset: number, length: number): Promise<ResourceBytes>
  closePackage(packageId: number): Promise<void>
  activityPackages(): Promise<PackageHistory[]>
  workspaceStatus(): Promise<WorkspaceStatus>
  workspaceFolders(): Promise<WorkspaceFolder[]>
}

export function localDemoConfig(): LocalDemoConfig | null {
  if (isTauri()) return null
  try {
    const raw = window.localStorage.getItem('openscp:local-key')
    if (!raw) return null
    const value = JSON.parse(raw) as Partial<LocalDemoConfig>
    return value.version === 1 && value.mode === 'mock' ? { version: 1, mode: 'mock' } : null
  } catch { return null }
}

export function createDataSource(): OpenScpDataSource { return isTauri() ? tauriDataSource() : mockDataSource() }

function tauriDataSource(): OpenScpDataSource {
  const unavailable = async () => []
  return {
    async overview() {
      const [packages, operations, events] = await Promise.all([tauriApi.activity.packages(), tauriApi.activity.operations(), tauriApi.activity.events()])
      return { packages: packages.length, resources: packages.reduce((total, item) => total + item.entryCount, 0), assets: 0, failedOperations: operations.filter((item) => item.status === 'failed').length, recentPackages: packages, recentEvents: events }
    },
    listGameTree: unavailable,
    listPackageFiles: unavailable,
    openPackage: tauriApi.packages.open,
    listResources: tauriApi.packages.listResources,
    readResourceBytes: tauriApi.packages.readBytes,
    closePackage: tauriApi.packages.close,
    activityPackages: tauriApi.activity.packages,
    workspaceStatus: tauriApi.workspace.get,
    workspaceFolders: tauriApi.workspace.list
  }
}

function mockDataSource(): OpenScpDataSource {
  const entries = mockResources()
  let nextPackageId = 1
  const folders: GameFolder[] = [
    { path: 'SimCityData', name: 'SimCityData', packageCount: 8, children: [
      { path: 'SimCityData/Locale', name: 'Locale', packageCount: 1, children: [] },
      { path: 'SimCityData/Packages', name: 'Packages', packageCount: 3, children: [] }
    ] },
    { path: 'SimCityUserData', name: 'SimCityUserData', packageCount: 4, children: [{ path: 'SimCityUserData/Cache', name: 'Cache', packageCount: 2, children: [] }] }
  ]
  const files: Record<string, PackageFile[]> = {
    SimCityData: ['SimCity_App.package', 'SimCity_Game.package', 'SimCity_Graphics.package', 'SimCity_Audio_Banks.package', 'SimCity_Audio_Streams.package', 'SimCity_Audio_MusicStreams.package', 'SimCity_DLC0.package', 'SimCityDataEP1.package'].map((name, index) => ({ name, path: `D:/ea-games/SimCity/SimCityData/${name}`, size: 40_000_000 + index * 12_000_000 })),
    'SimCityData/Locale': [{ name: 'Data.package', path: 'D:/ea-games/SimCity/SimCityData/Locale/zh-tw/Data.package', size: 16_000_000 }],
    'SimCityData/Packages': ['Terrain.package', 'Props.package', 'Buildings.package'].map((name, index) => ({ name, path: `D:/ea-games/SimCity/SimCityData/Packages/${name}`, size: 20_000_000 + index * 4_000_000 })),
    SimCityUserData: ['Cache.package', 'GraphicsCache.package', 'Server.package', 'LocalSettings.package'].map((name, index) => ({ name, path: `D:/ea-games/SimCity/SimCityUserData/${name}`, size: 10_000_000 + index * 2_000_000 }))
  }
  return {
    async overview() { return mockOverview },
    async listGameTree() { return folders },
    async listPackageFiles(folder) { return files[folder] ?? [] },
    async openPackage(path) { const packageId = nextPackageId++; return { package: { packageId, path, size: 391_000_000, kind: 'Dbpf', majorVersion: 1, minorVersion: 0, entryCount: entries.length }, resources: { items: entries.slice(0, 100), total: entries.length, offset: 0, limit: 100 } } },
    async listResources(_packageId, offset, limit, filter) { const filtered = filter ? entries.filter((item) => tgiText(item.tgi).includes(filter.toLowerCase())) : entries; return { items: filtered.slice(offset, offset + limit), total: filtered.length, offset, limit } },
    async readResourceBytes(_packageId, tgi, offset, length) { return { packageId: 1, tgi, offset, totalLength: 4096, bytes: Array.from({ length }, (_, index) => (index + offset) % 256) } },
    async closePackage() {},
    async activityPackages() { return mockOverview.recentPackages },
    async workspaceStatus() { return { configured: true, rootPath: 'D:/ea-games/SimCity', available: true } },
    async workspaceFolders() { return [{ relativePath: 'mods/example', readmeRelativePath: 'mods/example/README.md' }, { relativePath: 'assets/arcology', readmeRelativePath: 'assets/arcology/README.md' }] }
  }
}

function mockResources() { return Array.from({ length: 2489 }, (_, index) => ({ tgi: { typeId: 0x2f4e681b, group: index % 16, instance: 0x10000000 + index }, offset: 1024 + index * 64, storedSize: 64, decompressedSize: 64, compressed: index % 3 === 0 })) }
function tgiText(tgi: Tgi) { return `${tgi.typeId.toString(16)}:${tgi.group.toString(16)}:${tgi.instance.toString(16)}` }
