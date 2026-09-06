import { open } from "@tauri-apps/plugin-dialog";
import axios from "axios";
import { registerInterceptors } from "./interceptors";
import {
  command,
  subscribe,
  type ActivityEvent,
  type ExportProgress,
  type ExportStatus,
  type GameDirectoryDetection,
  type GameFolder,
  type MarkdownDocument,
  type MediaTools,
  type OpenPackageResponse,
  type Operation,
  type PackageFile,
  type PackageHistory,
  type ResourceBytes,
  type ResourceData,
  type PropertyResourceData,
  type Rw4ResourceData,
  type Rw4SectionDetail,
  type ResourcePage,
  type ResolvedResourceName,
  type SettingsStatus,
  type Tgi,
  type WorkspaceEntry,
  type WorkspaceStatus,
} from "./tauri";
import type { UnlistenFn } from "@tauri-apps/api/event";

export type {
  ActivityEvent,
  ExportProgress,
  ExportStatus,
  GameDirectoryDetection,
  GameFolder,
  MarkdownDocument,
  MediaTools,
  OpenPackageResponse,
  Operation,
  PackageFile,
  PackageHistory,
  ResourceBytes,
  ResourceData,
  PropertyResourceData,
  Rw4ResourceData,
  Rw4SectionDetail,
  ResourcePage,
  ResourcePreview,
  ResourceSummary,
  ResolvedResourceName,
  SettingsStatus,
  Tgi,
  WorkspaceEntry,
  WorkspaceStatus,
} from "./tauri";
export const $request = axios.create({ baseURL: "/api/v1", timeout: 15_000 });
registerInterceptors($request);

export const tauriApi = {
  settings: {
    get: () => command<SettingsStatus>("settings_get"),
    setGameDirectory: (path: string) =>
      command<SettingsStatus>("settings_set_game_directory", {
        request: { path },
      }),
    detectGameDirectory: () =>
      command<GameDirectoryDetection>("game_directory_detect"),
  },
  workspace: {
    get: () => command<WorkspaceStatus>("workspace_get"),
    pickDirectory: (title = "选择 OpenSCP 文档工作区") =>
      open({ directory: true, multiple: false, title }).then((path) =>
        typeof path === "string" ? path : null,
      ),
    setRoot: (path: string) =>
      command<WorkspaceStatus>("workspace_set_root", { request: { path } }),
    list: () => command<WorkspaceEntry[]>("workspace_list"),
    createFolder: (relativePath: string) =>
      command<WorkspaceEntry[]>("workspace_create_folder", {
        request: { relativePath },
      }),
    readMarkdown: (relativePath: string) =>
      command<MarkdownDocument>("workspace_read_markdown", {
        request: { relativePath },
      }),
    writeMarkdown: (
      relativePath: string,
      content: string,
      expectedRevision?: string,
    ) =>
      command<MarkdownDocument>("workspace_write_markdown", {
        request: { relativePath, content, expectedRevision },
      }),
    createMarkdown: (relativePath: string, content: string) =>
      command<MarkdownDocument>("workspace_create_markdown", {
        request: { relativePath, content },
      }),
    rename: (relativePath: string, newName: string) =>
      command<WorkspaceEntry[]>("workspace_rename", {
        request: { relativePath, newName },
      }),
    move: (relativePath: string, targetDirectory: string) =>
      command<WorkspaceEntry[]>("workspace_move", {
        request: { relativePath, targetDirectory },
      }),
  },
  packages: {
    listGameTree: (root: string) =>
      command<GameFolder[]>("list_game_tree", { request: { root } }),
    listPackageFiles: (root: string) =>
      command<PackageFile[]>("list_package_files", { request: { root } }),
    open: (path: string) =>
      command<OpenPackageResponse>("open_package", { request: { path } }),
    listResources: (
      packageId: number,
      offset: number,
      limit: number,
      filter?: string,
      typeId?: number,
    ) =>
      command<ResourcePage>("list_resources", {
        request: { packageId, offset, limit, filter, typeId },
      }),
    readBytes: (packageId: number, tgi: Tgi, offset: number, length: number) =>
      command<ResourceBytes>("read_resource_bytes", {
        request: { packageId, tgi, offset, length },
      }),
    readData: (packageId: number, tgi: Tgi) =>
      command<ResourceData>("read_resource_data", {
        request: { packageId, tgi },
      }),
    readPropertyPreview: (packageId: number, tgi: Tgi) =>
      command<PropertyResourceData>("read_property_preview", {
        request: { packageId, tgi },
      }),
    readRw4Preview: (packageId: number, tgi: Tgi) =>
      command<Rw4ResourceData>("read_rw4_preview", {
        request: { packageId, tgi },
      }),
    readRw4Section: (packageId: number, tgi: Tgi, number: number) =>
      command<Rw4SectionDetail>("read_rw4_section_detail", {
        request: { packageId, tgi, number },
      }),
    resolveNames: (packageId: number, tgis: Tgi[]) =>
      command<ResolvedResourceName[]>("resolve_names", {
        request: { packageId, tgis },
      }),
    close: (packageId: number) => command<void>("close_package", { packageId }),
    export: (packageId: number, tgi: Tgi, format: string, outputPath: string) =>
      command<{ jobId: number; outputPath: string }>("export", {
        request: { packageId, tgi, format, outputPath },
      }),
    exportStatus: (jobId: number) =>
      command<ExportStatus>("export_status", { jobId }),
    mediaTools: () => command<MediaTools>("detect_media_tools"),
  },
  activity: {
    operations: (limit = 100) =>
      command<Operation[]>("activity_list_operations", { limit }),
    events: (limit = 100) =>
      command<ActivityEvent[]>("activity_list_events", { limit }),
    packages: (limit = 100) =>
      command<PackageHistory[]>("activity_list_packages", { limit }),
  },
};

export const onExportProgress = (
  callback: (event: ExportProgress) => void,
): Promise<UnlistenFn> => subscribe("export:progress", callback);
export const isTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
