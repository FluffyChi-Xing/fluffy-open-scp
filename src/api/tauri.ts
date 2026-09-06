import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface CommandErrorShape {
  code: string;
  message: string;
}
export class TauriUnavailableError extends Error {
  constructor() {
    super("Tauri runtime is unavailable");
    this.name = "TauriUnavailableError";
  }
}

function inTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
export async function command<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!inTauriRuntime()) throw new TauriUnavailableError();
  try {
    return await invoke<T>(name, args);
  } catch (error) {
    if (
      error &&
      typeof error === "object" &&
      "code" in error &&
      "message" in error
    )
      throw error as CommandErrorShape;
    throw {
      code: "command_failed",
      message: error instanceof Error ? error.message : String(error),
    } satisfies CommandErrorShape;
  }
}
export function subscribe<T>(
  name: string,
  callback: (payload: T) => void,
): Promise<UnlistenFn> {
  if (!inTauriRuntime()) return Promise.reject(new TauriUnavailableError());
  return listen<T>(name, (event) => callback(event.payload));
}

export interface Operation {
  id: number;
  kind: string;
  target: string;
  status: string;
  durationMs?: number;
  bytesIn?: number;
  bytesOut?: number;
  detail?: unknown;
  createdAt: number;
  finishedAt?: number;
}
export interface ActivityEvent {
  id: number;
  operationId?: number;
  level: string;
  topic: string;
  message: string;
  payload?: unknown;
  createdAt: number;
}
export interface PackageHistory {
  id: number;
  path: string;
  size: number;
  entryCount: number;
  version: number;
  openCount: number;
  lastOpenedAt: number;
}
export interface Tgi {
  typeId: number;
  group: number;
  instance: number;
}
export interface ResourceSummary {
  tgi: Tgi;
  offset: number;
  storedSize: number;
  decompressedSize: number;
  compressed: boolean;
}
export interface TypeCountInfo {
  typeId: number;
  name: string;
  count: number;
}
export interface ResourcePage {
  items: ResourceSummary[];
  total: number;
  offset: number;
  limit: number;
  typeCounts: TypeCountInfo[];
}
export interface PackageSummary {
  packageId: number;
  path: string;
  size: number;
  kind: string;
  majorVersion: number;
  minorVersion: number;
  entryCount: number;
}
export interface PackageFile {
  path: string;
  name: string;
  size: number;
}
export interface GameFolder {
  path: string;
  name: string;
  children: GameFolder[];
  files: PackageFile[];
  packageCount: number;
}
export interface OpenPackageResponse {
  package: PackageSummary;
  resources: ResourcePage;
}
export interface ResourceBytes {
  packageId: number;
  tgi: Tgi;
  offset: number;
  totalLength: number;
  bytes: number[];
}
export interface ResourceData {
  totalLength: number;
  dataBase64: string;
}
export interface ResolvedResourceName {
  tgi: Tgi;
  displayName: string | null;
}
export interface PreviewData {
  offset: number;
  totalLength: number;
  bytes: number[];
}
export interface TextPreview extends PreviewData {
  kind: "text";
  content: string;
  encoding: string;
  language: string;
  truncated: boolean;
}
export interface HexPreview extends PreviewData {
  kind: "hex";
}
export interface ImagePreview extends PreviewData {
  kind: "image";
  src: string;
  mime: string;
  width?: number;
  height?: number;
}
export interface UnsupportedPreview extends PreviewData {
  kind: "unsupported";
  reason: string;
}
export type ResourcePreview =
  TextPreview | HexPreview | ImagePreview | UnsupportedPreview;
export interface MediaTool {
  available: boolean;
  path?: string;
  source?: string;
}
export interface MediaTools {
  vgmstream: MediaTool;
  ffmpeg: MediaTool;
}
export interface ExportStatus {
  jobId: number;
  phase: string;
  outputPath: string;
  error?: string;
}
export interface ExportProgress {
  jobId: number;
  phase: string;
  completed: number;
  total: number;
  tgi?: Tgi;
  outputPath: string;
}
export interface WorkspaceStatus {
  configured: boolean;
  rootPath?: string;
  available: boolean;
}
export interface WorkspaceEntry {
  relativePath: string;
  kind: "folder" | "file";
}
export interface MarkdownDocument {
  relativePath: string;
  content: string;
  size: number;
  revision: string;
}
export interface GameDirectoryStatus {
  path: string;
  exists: boolean;
  isDirectory: boolean;
  accessible: boolean;
  hasPackageMarker: boolean;
}
export interface SettingsStatus {
  gameDataPath?: string;
  gameDirectory?: GameDirectoryStatus;
}
export interface GameDirectoryDetection {
  candidates: GameDirectoryStatus[];
}

export const activityEventName = "activity:event";
export const exportProgressEventName = "export:progress";
