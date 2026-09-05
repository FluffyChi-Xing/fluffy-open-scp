import axios from 'axios'
import { registerInterceptors } from './interceptors'
import { command, subscribe, type ActivityEvent, type ExportProgress, type ExportStatus, type GameDirectoryDetection, type MarkdownDocument, type MediaTools, type OpenPackageResponse, type Operation, type PackageHistory, type ResourceBytes, type ResourcePage, type SettingsStatus, type Tgi, type WorkspaceFolder, type WorkspaceStatus } from './tauri'
import type { UnlistenFn } from '@tauri-apps/api/event'

export type { ActivityEvent, ExportProgress, ExportStatus, GameDirectoryDetection, MarkdownDocument, MediaTools, OpenPackageResponse, PackageHistory, ResourceBytes, ResourcePage, ResourceSummary, SettingsStatus, Tgi, WorkspaceFolder, WorkspaceStatus } from './tauri'
export const $request = axios.create({ baseURL: '/api/v1', timeout: 15_000 })
registerInterceptors($request)

export const tauriApi = {
  settings: {
    get: () => command<SettingsStatus>('settings_get'),
    setGameDirectory: (path: string) => command<SettingsStatus>('settings_set_game_directory', { request: { path } }),
    detectGameDirectory: () => command<GameDirectoryDetection>('game_directory_detect')
  },
  workspace: {
    get: () => command<WorkspaceStatus>('workspace_get'),
    setRoot: (path: string) => command<WorkspaceStatus>('workspace_set_root', { request: { path } }),
    list: () => command<WorkspaceFolder[]>('workspace_list'),
    createFolder: (relativePath: string) => command<WorkspaceFolder[]>('workspace_create_folder', { request: { relativePath } }),
    readMarkdown: (relativePath: string) => command<MarkdownDocument>('workspace_read_markdown', { request: { relativePath } }),
    writeMarkdown: (relativePath: string, content: string, expectedRevision?: string) => command<MarkdownDocument>('workspace_write_markdown', { request: { relativePath, content, expectedRevision } }),
    createMarkdown: (relativePath: string, content: string) => command<MarkdownDocument>('workspace_create_markdown', { request: { relativePath, content } })
  },
  packages: {
    open: (path: string) => command<OpenPackageResponse>('open_package', { request: { path } }),
    listResources: (packageId: number, offset: number, limit: number, filter?: string) => command<ResourcePage>('list_resources', { request: { packageId, offset, limit, filter } }),
    readBytes: (packageId: number, tgi: Tgi, offset: number, length: number) => command<ResourceBytes>('read_resource_bytes', { request: { packageId, tgi, offset, length } }),
    close: (packageId: number) => command<void>('close_package', { packageId }),
    export: (packageId: number, tgi: Tgi, format: string, outputPath: string) => command<{ jobId: number; outputPath: string }>('export', { request: { packageId, tgi, format, outputPath } }),
    exportStatus: (jobId: number) => command<ExportStatus>('export_status', { jobId }),
    mediaTools: () => command<MediaTools>('detect_media_tools')
  },
  activity: {
    operations: (limit = 100) => command<Operation[]>('activity_list_operations', { limit }),
    events: (limit = 100) => command<ActivityEvent[]>('activity_list_events', { limit }),
    packages: (limit = 100) => command<PackageHistory[]>('activity_list_packages', { limit })
  }
}

export const onExportProgress = (callback: (event: ExportProgress) => void): Promise<UnlistenFn> => subscribe('export:progress', callback)
export const isTauri = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
