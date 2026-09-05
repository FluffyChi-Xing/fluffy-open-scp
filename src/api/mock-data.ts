import type { ActivityEvent, Operation, PackageHistory } from './tauri'

export interface OverviewSnapshot {
  packages: number
  resources: number
  assets: number
  failedOperations: number
  recentPackages: PackageHistory[]
  recentEvents: ActivityEvent[]
}

export const mockOverview: OverviewSnapshot = {
  packages: 4,
  resources: 2489,
  assets: 122,
  failedOperations: 2,
  recentPackages: [
    { id: 1, path: 'D:/ea-games/SimCity/SimCity_App.package', size: 391_000_000, entryCount: 2489, version: 1, openCount: 12, lastOpenedAt: Date.now() - 1000 * 60 * 12 },
    { id: 2, path: 'D:/ea-games/SimCity/SimCity_DLC0.package', size: 84_000_000, entryCount: 438, version: 1, openCount: 5, lastOpenedAt: Date.now() - 1000 * 60 * 60 * 3 },
    { id: 3, path: 'D:/ea-games/SimCity/SimCity_Graphics.package', size: 126_000_000, entryCount: 786, version: 1, openCount: 3, lastOpenedAt: Date.now() - 1000 * 60 * 60 * 24 }
  ],
  recentEvents: [
    { id: 1, operationId: 1, level: 'info', topic: 'package', message: 'Package index ready', createdAt: Date.now() - 1000 * 60 * 12 },
    { id: 2, operationId: 2, level: 'info', topic: 'resource', message: 'Resource preview loaded', createdAt: Date.now() - 1000 * 60 * 42 },
    { id: 3, operationId: 3, level: 'warn', topic: 'export', message: 'Media tool is unavailable', createdAt: Date.now() - 1000 * 60 * 60 * 2 }
  ]
}

export const mockOperations: Operation[] = [
  { id: 1, kind: 'package_open', target: 'SimCity_App.package', status: 'success', durationMs: 84, bytesIn: 391_000_000, bytesOut: 0, createdAt: Date.now() - 1000 * 60 * 12, finishedAt: Date.now() - 1000 * 60 * 12 },
  { id: 2, kind: 'export', target: 'model.glb', status: 'failed', durationMs: 210, createdAt: Date.now() - 1000 * 60 * 60 * 2, finishedAt: Date.now() - 1000 * 60 * 60 * 2 }
]
