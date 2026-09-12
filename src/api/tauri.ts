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
export interface PropertyResourceData {
  claimedCount: number;
  entries: PropertyEntry[];
}
export interface Rw4ResourceData {
  fileType: string;
  sections: Rw4Section[];
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
  packageId?: number;
  tgi?: Tgi;
  src: string;
  mime: string;
  width?: number;
  height?: number;
  /** 像素风渲染（raster 等低分辨率纹理放大时保持锐利边缘）。 */
  pixelated?: boolean;
}
export interface AudioPreview extends PreviewData {
  kind: "audio";
  packageId?: number;
  tgi?: Tgi;
  src: string | null;
  mime: "audio/wav";
  toolAvailable: boolean;
  toolName: string;
  installCommand: string;
  outputBytes?: number;
}
export interface VideoPreview extends PreviewData {
  kind: "video";
  packageId?: number;
  tgi?: Tgi;
  src: string | null;
  mime: "video/mp4";
  toolAvailable: boolean;
  toolName: string;
  installCommand: string;
  outputBytes?: number;
}
export interface UnsupportedPreview extends PreviewData {
  kind: "unsupported";
  reason: string;
}

export interface PropertyEntry {
  hash: number;
  name: string | null;
  typeName: string;
  value: string;
  arrayLen: number | null;
}
export interface PropertyPreview extends PreviewData {
  kind: "property";
  packageId: number;
  tgi: Tgi;
  claimedCount: number;
  entries: PropertyEntry[];
}
/**
 * Decal Dictionary（贴花图鉴）：GroupContainer 低 16 位为 0xB185 / 0x1651 /
 * 0x1652 的 Property 资源。条目由 7 个并行数组按下标组装，每个条目的
 * RasterFileID 指向一个 Raster 资源，用条目自带的四色还原量化图。
 */
export interface DecalKey {
  instance: number;
  typeId: number;
  group: number;
}
export interface DecalRasterStatus {
  found: boolean;
  width: number | null;
  height: number | null;
  pixelFormat: number | null;
  /** pixFmt 21 且四色齐全时可解码为 PNG。 */
  decodable: boolean;
  sourcePackage: string | null;
  /** 不可解码原因，用于单元格占位文案。 */
  reason: string | null;
}
export interface DecalEntryMeta {
  index: number;
  id: DecalKey | null;
  raster: DecalKey | null;
  aspectRatio: number | null;
  /** Color1..4 原始 Vector4（线性分量是 XYZ 的一半）。 */
  colors: ([number, number, number, number] | null)[];
  /** Color1..4 的预览 RGBA8（线性 → sRGB）。 */
  colorsRgba8: [number, number, number, number][] | null;
  rasterStatus: DecalRasterStatus;
}
export interface DecalArrayLength {
  hash: number;
  length: number | null;
}
export interface DecalDictionaryData {
  material: DecalKey | null;
  textureSize: [number, number] | null;
  atlasSize: [number, number] | null;
  entries: DecalEntryMeta[];
  arrayLengths: DecalArrayLength[];
  uniformArrays: boolean;
}
export interface DecalImageData {
  index: number;
  error: string | null;
  width: number | null;
  height: number | null;
  pngBase64: string | null;
}
export interface Rw4Section {
  number: number;
  typeCode: number;
  typeName: string | null;
  size: number;
}
export interface Rw4Preview extends PreviewData {
  kind: "rw4";
  packageId: number;
  tgi: Tgi;
  fileType: string;
  sections: Rw4Section[];
}
export interface Rw4MeshDetail {
  triangleCount: number;
  vertexCount: number;
  decodedTriangles: number;
  decodedVertices: number;
  exportable: boolean;
  boundsMin: [number, number, number] | null;
  boundsMax: [number, number, number] | null;
  objBase64: string | null;
}
export interface Rw4TextureDetail {
  width: number;
  height: number;
  mipCount: number;
  textureType: number;
  pngBase64: string;
}
export interface Rw4SectionDetail {
  number: number;
  typeCode: number;
  typeName: string | null;
  size: number;
  pos: number;
  mesh: Rw4MeshDetail | null;
  texture: Rw4TextureDetail | null;
  hexDump: string | null;
}
export interface UnitKeyDto {
  typeId: number;
  group: number;
  instance: number;
}
export interface UnitFieldDto {
  hash: number;
  typeName: string;
  value: string;
}
export interface UnitTransformDto {
  /** WPF Matrix3D 行主序 12 floats：行 1-3 基向量，行 4 平移。 */
  matrix: number[];
}
export interface LightUnit {
  kind: "light";
  index: number;
  transform: UnitTransformDto | null;
  lightType: "Point" | "Spot" | "Line" | null;
  color: [number, number, number] | null;
  outerRadius: number | null;
  innerRadius: number | null;
  diffuse: number | null;
  length: number | null;
  cullDistance: "Near" | "Mid" | "Far" | "Max" | null;
  isVolumetric: boolean | null;
  debugName: string | null;
  fields: UnitFieldDto[];
}
export interface EffectUnit {
  kind: "effect";
  index: number;
  transform: UnitTransformDto | null;
  effectId: UnitKeyDto | null;
  enabled: boolean | null;
  fields: UnitFieldDto[];
}
export interface DecalUnit {
  kind: "decal";
  index: number;
  category: number;
  transform: UnitTransformDto | null;
  scale: number | null;
  depth: number | null;
  materialData: [number, number, number] | null;
  fields: UnitFieldDto[];
}
export interface PropUnit {
  kind: "prop";
  index: number;
  bin: number;
  transform: UnitTransformDto | null;
  slot: number | null;
  fields: UnitFieldDto[];
}
export interface PathPointUnit {
  kind: "pathPoint";
  index: number;
  point: [number, number, number] | null;
  tangent: [number, number, number] | null;
  pointIndex: number | null;
  fields: UnitFieldDto[];
}
export interface SpawnerUnit {
  kind: "spawner";
  index: number;
  transform: UnitTransformDto | null;
  id: UnitKeyDto | null;
  fields: UnitFieldDto[];
}
export type LotUnitDto =
  | LightUnit
  | EffectUnit
  | DecalUnit
  | PropUnit
  | PathPointUnit
  | SpawnerUnit;
/** 单级 LOD 模型的资源位置（跨包解析；该级缺失为 null）。 */
export interface LotModelLodRef {
  packageId: number;
  tgi: Tgi;
}
export interface LotEditorSession {
  tgi: Tgi;
  assetName: string | null;
  modelAvailable: boolean;
  modelKey: Tgi | null;
  /** LOD1~LOD4 模型位置（index 0 = LOD1）；缺失级为 null。 */
  modelLods: (LotModelLodRef | null)[];
  lotSize: [number, number] | null;
  /** LotPlacementTransform（0x0DB7FB17）行主序 12 floats；地面矩形取其逆对齐。 */
  lotPlacement: number[] | null;
  /** LotMask 四色量化地面图 PNG（LotColor1-4 着色），无或不可解为 null。 */
  lotMaskPng: string | null;
  /** LotMask 原始通道权重图（未阈值化；软混合用）。 */
  lotMaskRawPng: string | null;
  /** "Lot Textures" 地表共享纹理（DXT5 解码 PNG 裸 base64）。 */
  lotSurfacePng: string | null;
  /** LotColor1-4 的 RGBA（A = 地面贴图索引 0-15）。 */
  lotColors: [number, number, number, number][];
  /** LotColor1-4 是否实际存在（false = 黑/红/绿/蓝回退，不应着色）。 */
  lotColorsAuthored: boolean[];
  units: LotUnitDto[];
  pathPairs: number[];
  diagnostics: string[];
}
/** 单个材质的贴图集（官方 Material Set 通道拆分，§27 源码实证语义）。 */
export interface LotMaterialTextures {
  /** slot1 漫反射贴图（仅无 slot0 参数表的 simple diffuse 材质下发）。 */
  baseColorPng: Uint8Array<ArrayBuffer> | null;
  /** slot2 法线（标准切线空间 RGB，B=沿法线轴；A=spec）。 */
  normalPng: Uint8Array<ArrayBuffer> | null;
  /** slot3 shader map B 反转 = 粗糙度灰度。 */
  roughnessPng: Uint8Array<ArrayBuffer> | null;
  /** slot2 alpha = AO 灰度。 */
  aoPng: Uint8Array<ArrayBuffer> | null;
  /** slot1 原始 color control map（tint 着色器查表键）。 */
  tintPng: Uint8Array<ArrayBuffer> | null;
  /** slot4 原始 256×8 tint palette。 */
  palettePng: Uint8Array<ArrayBuffer> | null;
  /** slot3 原始 shader map（源码语义：B=specularity，A=窗洞/Interior 位置）。 */
  shaderPng: Uint8Array<ArrayBuffer> | null;
  /** slot5 原始 interior map（预渲染房间图集，alpha=逐窗灯亮通道）。 */
  interiorPng: Uint8Array<ArrayBuffer> | null;
  /** slot5 alpha = relief 高度灰度（kFlatLevel=23 平面钳制）。 */
  reliefPng: Uint8Array<ArrayBuffer> | null;
  /**
   * slot0 参数表 f32（cols×4 float4，源码行绑定）：row0=(palU,palU2,
   * interiorScale,interiorOffset)、row1=regionXform(base)、row2=regionXform2(top)、
   * row3=(tilePadding,interiorRoomInvSize)。palette V=buildingVariation
   * 实例行不在表内。
   */
  paramsF32: Float32Array | null;
  paramCols: number;
}
/**
 * PE 精细渲染：`read_lot_model_meshes` 原始字节容器解析结果（v7）。
 * 容器（小端）：`magic("LOTM") | version=8 | mesh_count`，每 mesh
 * `u32 len + GLB`（COLOR_0 烘焙 + TEXCOORD_1.xy=materialIndex/255+内景种子 +
 * TEXCOORD_2/3=facade 世界投影 UV）；
 * `material_count`，每材质 9 张 PNG（base/normal/rough/ao/tintRaw/palette/
 * shaderMap/interiorMap/reliefMap）+ 参数表 f32 + paramCols；每 mesh
 * `u32 material_index + u8 uv_kind`（0 无 / 1 常规贴图 / 2 tint 着色器）；
 * 末尾 `u32 diag_len + UTF-8` 槽位诊断文本（mesh↔material↔slot 贴图及来源包）。
 */
export interface LotModelPayload {
  /** 每个网格一个 GLB ArrayBuffer。 */
  glbs: ArrayBuffer[];
  /** 逐材质贴图集（去重后）。 */
  materials: LotMaterialTextures[];
  /** 每 mesh 的材质下标（与 glbs 同序）。 */
  meshMaterialIndices: number[];
  /** 每 mesh UV 类型（0 无 / 1 常规贴图 / 2 tint 着色器）。 */
  meshUvKinds: number[];
  /** 模型槽位诊断文本（info 面板展示/复制）。 */
  diagnostics: string;
}
export interface RasterPreviewData {
  rasterType: number;
  width: number;
  height: number;
  mipCount: number;
  pixelSize: number;
  pixelFormat: number;
  /** pixFmt 21（D3DFMT_A8R8G8B8，未压缩）可解码为 PNG；压缩变体仅元数据。 */
  decodable: boolean;
  pngBase64: string | null;
}
/** TGA / CUR(ICO) / Greyscale Map 通用解码结果（read_image_preview）。 */
export interface GenericImagePreviewData {
  imageKind: string;
  width: number;
  height: number;
  pngBase64: string;
}
export interface FontPreview extends PreviewData {
  kind: "font";
  packageId?: number;
  tgi?: Tgi;
  src: string;
  totalBytes: number;
}
export type ResourcePreview =
  | TextPreview
  | HexPreview
  | ImagePreview
  | UnsupportedPreview
  | AudioPreview
  | VideoPreview
  | PropertyPreview
  | Rw4Preview
  | FontPreview;
export interface ResourceAnnotation {
  id: number;
  packagePath: string;
  typeId: number;
  groupId: number;
  instance: number;
  topic: string;
  title: string;
  content: string;
  createdAt: number;
  updatedAt: number;
}
export interface ResourceAnnotationInput {
  packagePath: string;
  typeId: number;
  groupId: number;
  instance: number;
  topic: string;
  title: string;
  content: string;
}
export interface AnnotationTopicStat {
  topic: string;
  annotationCount: number;
  resourceCount: number;
}
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
export interface ExtensionStat {
  ext: string;
  typeId: number;
  known: boolean;
  count: number;
  storedSize: number;
  decompressedSize: number;
}
export interface PackageStat {
  path: string;
  name: string;
  fileSize: number;
  entryCount: number;
  extensions: ExtensionStat[];
  knownDecompressed: number;
  unknownDecompressed: number;
}
export interface TotalsStat {
  fileSize: number;
  storedSize: number;
  decompressedSize: number;
  knownDecompressed: number;
  unknownDecompressed: number;
  knownCount: number;
  unknownCount: number;
  entryCount: number;
}
export interface PackageStatistics {
  packages: PackageStat[];
  extensions: ExtensionStat[];
  totals: TotalsStat;
  failed: string[];
}

export interface OverrideChainItem {
  path: string;
  name: string;
  decompressedSize: number;
  wins: boolean;
}
export interface OverrideConflict {
  typeId: number;
  groupId: number;
  instanceId: number;
  ext: string;
  chain: OverrideChainItem[];
}
export interface OverridePackageStat {
  path: string;
  name: string;
  entryCount: number;
  overrides: number;
}
export interface OverrideScanStats {
  packages: number;
  entries: number;
  duplicatedTgis: number;
  overriddenEntries: number;
}
export interface OverrideScanResponse {
  conflicts: OverrideConflict[];
  packages: OverridePackageStat[];
  stats: OverrideScanStats;
  failed: string[];
}

export interface LocaleTgi {
  typeId: number;
  group: number;
  instance: number;
}
export interface LocaleTableSummary {
  tgi: LocaleTgi;
  tableId: number;
  stringCount: number;
  sample: string[];
}
export interface LocaleTablesResponse {
  tables: LocaleTableSummary[];
}
export interface LocaleItem {
  key: string;
  id: number | null;
  text: string;
}
export interface LocaleItemsResponse {
  tgi: LocaleTgi;
  items: LocaleItem[];
}
export interface LocaleTableEdit {
  tgi: LocaleTgi;
  items: LocaleItem[];
}
export interface WriteLocaleOverlayResponse {
  outputPath: string;
  entryCount: number;
  bytesWritten: number;
}

export const activityEventName = "activity:event";
export const exportProgressEventName = "export:progress";
