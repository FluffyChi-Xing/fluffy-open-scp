import { convertFileSrc } from "@tauri-apps/api/core";
import { join, tempDir } from "@tauri-apps/api/path";
import { isTauri, tauriApi } from "./index";
import type {
  AudioPreview,
  GameFolder,
  VideoPreview,
  OpenPackageResponse,
  PackageFile,
  PackageHistory,
  PackageStatistics,
  PropertyResourceData,
  DecalDictionaryData,
  DecalImageData,
  LotEditorSession,
  RasterPreviewData,
  ResourceBytes,
  ResourcePage,
  ResourcePreview,
  ResourceSummary,
  ResolvedResourceName,
  Rw4ResourceData,
  Rw4SectionDetail,
  Tgi,
  WorkspaceEntry,
  WorkspaceStatus,
  TypeCountInfo,
} from "./tauri";
import { mockOverview } from "./mock-data";
import { decodeTextBytes, looksLikeText, stripLocaleJsonPrefix } from "@/lib/text-decode";
import {
  imageMimeForType,
  AUDIO_TYPE_ID,
  PROPERTY_TYPE_ID,
  RASTER_TYPE_ID,
  RW4_TYPE_ID,
  VIDEO_TYPE_ID,
  WWISE_BANK_TYPE_ID,
  GENERIC_IMAGE_TYPE_IDS,
  TTF_TYPE_ID,
  textPreviewLanguage,
} from "@/lib/resource-types";

export interface LocalDemoConfig {
  version: 1;
  mode: "mock";
}
export interface OpenScpDataSource {
  overview(): Promise<typeof mockOverview>;
  listGameTree(root: string): Promise<GameFolder[]>;
  listPackageFiles(folder: string): Promise<PackageFile[]>;
  openPackage(path: string): Promise<OpenPackageResponse>;
  listResources(
    packageId: number,
    offset: number,
    limit: number,
    filter?: string,
    typeId?: number,
  ): Promise<ResourcePage>;
  readResourceBytes(
    packageId: number,
    tgi: Tgi,
    offset: number,
    length: number,
  ): Promise<ResourceBytes>;
  resolveNames(packageId: number, tgis: Tgi[]): Promise<ResolvedResourceName[]>;
  previewResource(
    packageId: number,
    resource: ResourceSummary,
  ): Promise<ResourcePreview>;
  readPropertyPreview(
    packageId: number,
    tgi: Tgi,
  ): Promise<PropertyResourceData>;
  /** Decal Dictionary 元数据与条目（不解码像素）。 */
  readDecalDictionary(
    packageId: number,
    tgi: Tgi,
  ): Promise<DecalDictionaryData>;
  /** 按条目下标批量解码缩略图（单次上限 256）。 */
  readDecalImages(
    packageId: number,
    tgi: Tgi,
    indices: number[],
  ): Promise<DecalImageData[]>;
  readLotEditorSession(packageId: number, tgi: Tgi): Promise<LotEditorSession>;
  /** 返回 `read_lot_model_meshes` 原始字节容器（LotModelPayload，见 tauri.ts）。 */
  readLotModelMeshes(packageId: number, tgi: Tgi): Promise<ArrayBuffer>;
  /** 文本预览全量原始字节（服务端 8MB 上限，见 read_resource_text）。 */
  readResourceText(packageId: number, tgi: Tgi): Promise<ArrayBuffer>;
  readRasterPreview(packageId: number, tgi: Tgi): Promise<RasterPreviewData>;
  readRw4Preview(packageId: number, tgi: Tgi): Promise<Rw4ResourceData>;
  readRw4Section(
    packageId: number,
    tgi: Tgi,
    number: number,
  ): Promise<Rw4SectionDetail>;
  closePackage(packageId: number): Promise<void>;
  activityPackages(): Promise<PackageHistory[]>;
  packageStats(paths: string[]): Promise<PackageStatistics>;
  workspaceStatus(): Promise<WorkspaceStatus>;
  workspaceEntries(): Promise<WorkspaceEntry[]>;
}

export function localDemoConfig(): LocalDemoConfig | null {
  if (isTauri()) return null;
  try {
    const raw = window.localStorage.getItem("openscp:local-key");
    if (!raw) return null;
    const value = JSON.parse(raw) as Partial<LocalDemoConfig>;
    return value.version === 1 && value.mode === "mock"
      ? { version: 1, mode: "mock" }
      : null;
  } catch {
    return null;
  }
}

export function createDataSource(): OpenScpDataSource {
  return isTauri() ? tauriDataSource() : mockDataSource();
}

function tauriDataSource(): OpenScpDataSource {
  return {
    async overview() {
      const [packages, operations, events] = await Promise.all([
        tauriApi.activity.packages(),
        tauriApi.activity.operations(),
        tauriApi.activity.events(),
      ]);
      return {
        packages: packages.length,
        resources: packages.reduce((total, item) => total + item.entryCount, 0),
        assets: 0,
        failedOperations: operations.filter((item) => item.status === "failed")
          .length,
        recentPackages: packages,
        recentEvents: events,
      };
    },
    listGameTree: tauriApi.packages.listGameTree,
    listPackageFiles: tauriApi.packages.listPackageFiles,
    openPackage: tauriApi.packages.open,
    listResources: tauriApi.packages.listResources,
    readResourceBytes: tauriApi.packages.readBytes,
    resolveNames: tauriApi.packages.resolveNames,
    readPropertyPreview: tauriApi.packages.readPropertyPreview,
    readDecalDictionary: tauriApi.packages.readDecalDictionary,
    readDecalImages: tauriApi.packages.readDecalImages,
    readLotEditorSession: tauriApi.packages.readLotEditorSession,
    readLotModelMeshes: tauriApi.packages.readLotModelMeshes,
    readResourceText: tauriApi.packages.readResourceText,
    readRasterPreview: tauriApi.packages.readRasterPreview,
    readRw4Preview: tauriApi.packages.readRw4Preview,
    readRw4Section: tauriApi.packages.readRw4Section,
    previewResource: tauriPreview,
    closePackage: tauriApi.packages.close,
    activityPackages: tauriApi.activity.packages,
    packageStats: tauriApi.packages.statistics,
    workspaceStatus: tauriApi.workspace.get,
    workspaceEntries: tauriApi.workspace.list,
  };
}

function mockDataSource(): OpenScpDataSource {
  const entries = mockResources();
  let nextPackageId = 1;
  const filesByFolder: Record<string, PackageFile[]> = {
    "D:/ea-games/SimCity/SimCityData": [
      "SimCity_App.package",
      "SimCity_Game.package",
      "SimCity_Graphics.package",
      "SimCity_Audio_Banks.package",
      "SimCity_Audio_Streams.package",
      "SimCity_Audio_MusicStreams.package",
      "SimCity_DLC0.package",
      "SimCityDataEP1.package",
    ].map((name, index) => ({
      name,
      path: `D:/ea-games/SimCity/SimCityData/${name}`,
      size: 40_000_000 + index * 12_000_000,
    })),
    "D:/ea-games/SimCity/SimCityData/Locale": [
      {
        name: "Data.package",
        path: "D:/ea-games/SimCity/SimCityData/Locale/Data.package",
        size: 16_000_000,
      },
    ],
    "D:/ea-games/SimCity/SimCityData/Packages": [
      "Terrain.package",
      "Props.package",
      "Buildings.package",
    ].map((name, index) => ({
      name,
      path: `D:/ea-games/SimCity/SimCityData/Packages/${name}`,
      size: 20_000_000 + index * 4_000_000,
    })),
    "D:/ea-games/SimCity/SimCityUserData": [
      "Cache.package",
      "GraphicsCache.package",
      "Server.package",
      "LocalSettings.package",
    ].map((name, index) => ({
      name,
      path: `D:/ea-games/SimCity/SimCityUserData/${name}`,
      size: 10_000_000 + index * 2_000_000,
    })),
    "D:/ea-games/SimCity": [
      {
        name: "SimCity.exe",
        path: "D:/ea-games/SimCity/SimCity.exe",
        size: 31_488_000,
      },
      { name: "notes.txt", path: "D:/ea-games/SimCity/notes.txt", size: 2_048 },
    ],
  };
  function mockFolder(
    path: string,
    name: string,
    children: GameFolder[],
  ): GameFolder {
    const files = filesByFolder[path] ?? [];
    return {
      path,
      name,
      children,
      files,
      packageCount: files.filter((file) =>
        file.name.toLowerCase().endsWith(".package"),
      ).length,
    };
  }
  const folders: GameFolder[] = [
    mockFolder("D:/ea-games/SimCity", "SimCity", [
      mockFolder("D:/ea-games/SimCity/SimCityData", "SimCityData", [
        mockFolder("D:/ea-games/SimCity/SimCityData/Locale", "Locale", []),
        mockFolder("D:/ea-games/SimCity/SimCityData/Packages", "Packages", []),
      ]),
      mockFolder("D:/ea-games/SimCity/SimCityUserData", "SimCityUserData", [
        mockFolder("D:/ea-games/SimCity/SimCityUserData/Cache", "Cache", []),
      ]),
    ]),
  ];
  return {
    async overview() {
      return mockOverview;
    },
    async listGameTree() {
      return folders;
    },
    async listPackageFiles(folder) {
      return filesByFolder[folder] ?? [];
    },
    async openPackage(path) {
      const packageId = nextPackageId++;
      return {
        package: {
          packageId,
          path,
          size: 391_000_000,
          kind: "Dbpf",
          majorVersion: 1,
          minorVersion: 0,
          entryCount: entries.length,
        },
        resources: {
          items: entries.slice(0, 100),
          total: entries.length,
          offset: 0,
          limit: 100,
          typeCounts: mockTypeCounts(entries),
        },
      };
    },
    async listResources(_packageId, offset, limit, filter, typeId) {
      let filtered = filter
        ? entries.filter((item) =>
            tgiText(item.tgi).includes(filter.toLowerCase()),
          )
        : entries;
      if (typeId !== undefined)
        filtered = filtered.filter((item) => item.tgi.typeId === typeId);
      return {
        items: filtered.slice(offset, offset + limit),
        total: filtered.length,
        offset,
        limit,
        typeCounts: mockTypeCounts(entries),
      };
    },
    async readResourceBytes(_packageId, tgi, offset, length) {
      return {
        packageId: 1,
        tgi,
        offset,
        totalLength: 4096,
        bytes: Array.from({ length }, (_, index) => (index + offset) % 256),
      };
    },
    async resolveNames(_packageId, tgis) {
      return tgis.map((tgi) => ({
        tgi,
        displayName:
          tgi.instance % 3 === 0
            ? `Sample asset ${tgi.instance.toString(16).slice(-4)}`
            : null,
      }));
    },
    async previewResource(_packageId, resource) {
      const bytes = Array.from(
        { length: Math.min(256, resource.decompressedSize) },
        (_, index) => (index * 17) % 256,
      );
      const base = { offset: 0, totalLength: resource.decompressedSize, bytes };
      if (resource.tgi.typeId === 0x0a98eaf0)
        return {
          kind: "text",
          ...base,
          content: '{\\n  "locale": "zh-CN",\\n  "status": "ready"\\n}',
          encoding: "utf-8",
          language: "json",
          truncated: false,
        };
      if (resource.tgi.typeId === PROPERTY_TYPE_ID)
        return {
          kind: "property",
          packageId: _packageId,
          tgi: resource.tgi,
          claimedCount: mockPropertyEntries.length,
          entries: mockPropertyEntries,
          ...base,
        };
      if (resource.tgi.typeId === RW4_TYPE_ID)
        return {
          kind: "rw4",
          packageId: _packageId,
          tgi: resource.tgi,
          fileType: "Model",
          sections: mockRw4Sections,
          ...base,
        };
      if (imageMimeForType(resource.tgi.typeId) || resource.tgi.typeId === 0x2f4e681c)
        return {
          kind: "image",
          packageId: _packageId,
          tgi: resource.tgi,
          ...base,
          src: svgPreviewUrl(resource.tgi.instance),
          mime: "image/svg+xml",
          width: 320,
          height: 180,
        };
      if (resource.tgi.typeId === 0x376840d7) {
        return {
          kind: "video",
          packageId: _packageId,
          tgi: resource.tgi,
          ...base,
          src: null,
          mime: "video/mp4",
          toolAvailable: false,
          toolName: "ffmpeg",
          installCommand: "winget install --id Gyan.FFmpeg -e --source winget",
        };
      }
      if (
        resource.tgi.typeId === AUDIO_TYPE_ID ||
        resource.tgi.typeId === WWISE_BANK_TYPE_ID
      ) {
        return {
          kind: "audio",
          packageId: _packageId,
          tgi: resource.tgi,
          ...base,
          src: null,
          mime: "audio/wav",
          toolAvailable: false,
          toolName: "vgmstream",
          installCommand:
            "winget install --id vgmstream.vgmstream -e --source winget",
        };
      }
      return { kind: "hex", ...base };
    },
    async readPropertyPreview(_packageId, _tgi) {
      return {
        claimedCount: mockPropertyEntries.length,
        entries: mockPropertyEntries,
      };
    },
    // 演示模式没有 decal 字典数据；相册对空结果展示空态即可。
    async readDecalDictionary(_packageId, _tgi) {
      return {
        material: null,
        textureSize: null,
        atlasSize: null,
        entries: [],
        arrayLengths: [],
        uniformArrays: true,
      };
    },
    async readDecalImages(_packageId, _tgi, _indices) {
      return [];
    },
    async readLotEditorSession(_packageId, tgi) {
      const matrix = [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];
      return {
        tgi,
        assetName: "Mock Residential Tower",
        modelAvailable: true,
        modelKey: { typeId: 0x2f4e681b, group: 0, instance: 0x10000001 },
        modelLods: [
          { packageId: 1, tgi: { typeId: 0x2f4e681b, group: 0, instance: 0x10000001 } },
          { packageId: 1, tgi: { typeId: 0x2f4e681b, group: 0, instance: 0x10000002 } },
          { packageId: 1, tgi: { typeId: 0x2f4e681b, group: 0, instance: 0x10000003 } },
          null,
        ],
        lotSize: [136, 136],
        lotPlacement: null,
        lotColors: [
          [0, 0, 0, 0],
          [255, 0, 0, 1],
          [0, 255, 0, 2],
          [0, 0, 255, 3],
        ],
        lotColorsAuthored: [true, true, true, true],
        lotMaskPng: null,
        lotMaskRawPng: null,
        lotSurfacePng: null,
        units: [
          {
            kind: "light",
            index: 0,
            transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 6] },
            lightType: "Point",
            color: [1, 0.85, 0.6],
            outerRadius: 4,
            innerRadius: 1,
            diffuse: 1,
            length: null,
            cullDistance: "Mid",
            isVolumetric: false,
            debugName: "roof beacon",
            fields: [],
          },
          {
            kind: "light",
            index: 1,
            transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 3, 0, 2] },
            lightType: "Spot",
            color: [1, 1, 1],
            outerRadius: 2.5,
            innerRadius: 0.5,
            diffuse: 0.8,
            length: 5,
            cullDistance: "Far",
            isVolumetric: false,
            debugName: null,
            fields: [],
          },
          {
            kind: "decal",
            index: 0,
            category: 0,
            transform: { matrix: [...matrix.slice(0, 9), 2, 0, 0] },
            scale: 3,
            depth: 0.2,
            materialData: [1, 0, 0],
            fields: [],
          },
          {
            kind: "spawner",
            index: 0,
            transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, -2, 0, 1] },
            id: { typeId: 0x0, group: 0, instance: 0xcafe },
            fields: [],
          },
        ],
        pathPairs: [],
        diagnostics: [],
      } satisfies LotEditorSession;
    },
    async readRw4Preview(_packageId, _tgi) {
      return { fileType: "Model", sections: mockRw4Sections };
    },
    async readLotModelMeshes(_packageId, _tgi) {
      // 浏览器 demo 模式：合法空容器 v2（0 网格、0 材质）；GLB 构建在 Rust 导出器侧
      const out = new ArrayBuffer(16);
      const view = new DataView(out);
      view.setUint32(0, 0x4d544f4c, true); // "LOTM"
      view.setUint32(4, 4, true);
      view.setUint32(8, 0, true); // mesh_count
      view.setUint32(12, 0, true); // material_count
      return out;
    },
    async readResourceText(_packageId, _tgi) {
      // demo 模式文本内容直接内联在 previewResource，无全量通道
      return new ArrayBuffer(0);
    },
    async readRasterPreview(_packageId, _tgi) {
      return {
        rasterType: 2,
        width: 0,
        height: 0,
        mipCount: 0,
        pixelSize: 8,
        pixelFormat: 21,
        decodable: false,
        pngBase64: null,
      } satisfies RasterPreviewData;
    },
    async readRw4Section(_packageId, _tgi, number) {
      const section = mockRw4Sections.find((item) => item.number === number) ?? mockRw4Sections[0];
      return {
        number: section.number,
        typeCode: section.typeCode,
        typeName: section.typeName,
        size: section.size,
        pos: 1024 + section.number * 64,
        mesh:
          section.typeName === "Mesh"
            ? {
                triangleCount: 12,
                vertexCount: 8,
                decodedTriangles: 12,
                decodedVertices: 8,
                exportable: true,
                boundsMin: [-1, -1, 0],
                boundsMax: [1, 1, 0],
                objBase64: btoa(
                  [
                    "v -1 -1 0",
                    "v 1 -1 0",
                    "v 1 1 0",
                    "v -1 1 0",
                    "f 1 2 3",
                    "f 1 3 4",
                    "",
                  ].join("\n"),
                ),
              }
            : null,
        texture:
          section.typeName === "Texture"
            ? {
                width: 64,
                height: 64,
                mipCount: 1,
                textureType: 1,
                pngBase64: MOCK_PNG_BASE64,
              }
            : null,
        hexDump:
          section.typeName === "Mesh" || section.typeName === "Texture"
            ? null
            : "00000000  2F 2F 20 44 79 6E 61 6D 69 63 61 6C 6C 79 20 4C   // Dynamically L\n00000010  6F 61 64 65 64 20 44 4C 43 20 4A 61 76 61 53 63   oaded DLC JavaSc",
      };
    },
    async closePackage() {},
    async activityPackages() {
      return mockOverview.recentPackages;
    },
    async packageStats(paths) {
      // demo 模式：对每个请求路径生成确定性的伪统计，方便浏览器预览图表。
      const byExt = new Map<string, { count: number; size: number; known: boolean }>();
      let knownSize = 0;
      let unknownSize = 0;
      for (const item of entries) {
        const name = mockTypeNames[item.tgi.typeId];
        const known = name !== undefined;
        const ext = known ? name.replace(/\s+file$/i, "") : "UNKNOWN_TYPE";
        const size = item.decompressedSize;
        const slot = byExt.get(ext) ?? { count: 0, size: 0, known };
        slot.count += 1;
        slot.size += size;
        byExt.set(ext, slot);
        if (known) knownSize += size;
        else unknownSize += size;
      }
      const extensions = [...byExt.entries()]
        .map(([ext, slot]) => ({
          ext,
          typeId: 0,
          known: slot.known,
          count: slot.count,
          storedSize: slot.size,
          decompressedSize: slot.size,
        }))
        .sort((a, b) => b.decompressedSize - a.decompressedSize);
      const perPackage = (path: string, index: number) => {
        const scale = 0.6 + ((index * 7) % 5) * 0.2;
        return {
          path,
          name: path.split(/[\\/]/).pop() ?? path,
          fileSize: Math.round(391_000_000 * scale),
          entryCount: entries.length,
          extensions: extensions.map((item) => ({
            ...item,
            count: Math.max(1, Math.round(item.count * scale)),
            storedSize: Math.round(item.storedSize * scale),
            decompressedSize: Math.round(item.decompressedSize * scale),
          })),
          knownDecompressed: Math.round(knownSize * scale),
          unknownDecompressed: Math.round(unknownSize * scale),
        };
      };
      return {
        packages: (paths.length ? paths : ["SimCity_App.package", "SimCity_Game.package"]).map(
          perPackage,
        ),
        extensions,
        totals: {
          fileSize: 782_000_000,
          storedSize: knownSize + unknownSize,
          decompressedSize: knownSize + unknownSize,
          knownDecompressed: knownSize,
          unknownDecompressed: unknownSize,
          knownCount: entries.length - 40,
          unknownCount: 40,
          entryCount: entries.length,
        },
        failed: [],
      } satisfies PackageStatistics;
    },
    async workspaceStatus() {
      return {
        configured: true,
        rootPath: "D:/ea-games/SimCity",
        available: true,
      };
    },
    async workspaceEntries() {
      return [
        { relativePath: "mods", kind: "folder" },
        { relativePath: "mods/example", kind: "folder" },
        { relativePath: "mods/example/README.md", kind: "file" },
        { relativePath: "mods/example/notes.md", kind: "file" },
        { relativePath: "assets", kind: "folder" },
        { relativePath: "assets/arcology", kind: "folder" },
        { relativePath: "assets/arcology/README.md", kind: "file" },
        { relativePath: "README.md", kind: "file" },
      ] satisfies WorkspaceEntry[];
    },
  };
}

function base64ToBytes(base64: string): Uint8Array<ArrayBuffer> {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

async function waitForExport(jobId: number) {
  for (let attempt = 0; attempt < 600; attempt += 1) {
    const status = await tauriApi.packages.exportStatus(jobId);
    if (status.phase === "succeeded") return status;
    if (status.phase === "failed") {
      throw new Error(status.error ?? "媒体转换失败");
    }
    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
  throw new Error("媒体转换超时");
}

async function tauriMediaPreview(
  packageId: number,
  resource: ResourceSummary,
): Promise<AudioPreview | VideoPreview> {
  const isVideo = resource.tgi.typeId === VIDEO_TYPE_ID;
  const tools = await tauriApi.packages.mediaTools();
  const tool = isVideo ? tools.ffmpeg : tools.vgmstream;
  const kind = isVideo ? "video" : "audio";
  const installCommand = isVideo
    ? "winget install --id Gyan.FFmpeg -e --source winget"
    : "winget install --id vgmstream.vgmstream -e --source winget";
  const base = {
    packageId,
    tgi: resource.tgi,
    offset: 0,
    totalLength: resource.decompressedSize,
    bytes: [],
    src: null,
    mime: isVideo ? ("video/mp4" as const) : ("audio/wav" as const),
    toolAvailable: tool.available,
    toolName: isVideo ? "ffmpeg" : "vgmstream",
    installCommand,
  };
  if (!tool.available || !tool.path) {
    return { kind, ...base } as AudioPreview | VideoPreview;
  }
  const directory = await tempDir();
  const suffix = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const outputPath = await join(directory, `openscp-preview-${suffix}.${isVideo ? "mp4" : "wav"}`);
  const accepted = await tauriApi.packages.export(
    packageId,
    resource.tgi,
    isVideo ? "mp4" : "wav",
    outputPath,
  );
  await waitForExport(accepted.jobId);
  return {
    kind,
    ...base,
    src: convertFileSrc(outputPath),
    toolAvailable: true,
    outputBytes: undefined,
  } as AudioPreview | VideoPreview;
}
async function tauriPreview(
  packageId: number,
  resource: ResourceSummary,
): Promise<ResourcePreview> {
  if (
    resource.tgi.typeId === AUDIO_TYPE_ID ||
    resource.tgi.typeId === WWISE_BANK_TYPE_ID ||
    resource.tgi.typeId === VIDEO_TYPE_ID
  ) {
    return tauriMediaPreview(packageId, resource);
  }

  const mime = imageMimeForType(resource.tgi.typeId);
  if (mime) {
    const data = await tauriApi.packages.readData(packageId, resource.tgi);
    const blob = new Blob([base64ToBytes(data.dataBase64)], { type: mime });
    return {
      kind: "image",
      packageId,
      tgi: resource.tgi,
      offset: 0,
      totalLength: data.totalLength,
      bytes: [],
      src: URL.createObjectURL(blob),
      mime,
    };
  }
  if (resource.tgi.typeId === RASTER_TYPE_ID) {
    // Raster（0x2f4e681c）：pixFmt 21 未压缩可解为 PNG；压缩变体回退通用"暂不支持"。
    const data = await tauriApi.packages.readRasterPreview(
      packageId,
      resource.tgi,
    );
    if (data.decodable && data.pngBase64) {
      return {
        kind: "image",
        packageId,
        tgi: resource.tgi,
        offset: 0,
        totalLength: resource.decompressedSize,
        bytes: [],
        src: `data:image/png;base64,${data.pngBase64}`,
        mime: "image/png",
        width: data.width,
        height: data.height,
        pixelated: true,
      };
    }
    return {
      kind: "hex",
      offset: 0,
      totalLength: resource.decompressedSize,
      bytes: [],
    };
  }
  if ((GENERIC_IMAGE_TYPE_IDS as readonly number[]).includes(resource.tgi.typeId)) {
    // TGA / Cursor / Greyscale Map：Rust 侧解码为 PNG；失败回退 hex。
    try {
      const data = await tauriApi.packages.readImagePreview(
        packageId,
        resource.tgi,
      );
      return {
        kind: "image",
        packageId,
        tgi: resource.tgi,
        offset: 0,
        totalLength: resource.decompressedSize,
        bytes: [],
        src: `data:image/png;base64,${data.pngBase64}`,
        mime: "image/png",
        width: data.width,
        height: data.height,
        pixelated: true,
      };
    } catch {
      const fallback = await tauriApi.packages.readBytes(
        packageId,
        resource.tgi,
        0,
        Math.min(4096, resource.decompressedSize),
      );
      return { kind: "hex", ...fallback };
    }
  }
  if (resource.tgi.typeId === TTF_TYPE_ID) {
    const data = await tauriApi.packages.readData(packageId, resource.tgi);
    const blob = new Blob([base64ToBytes(data.dataBase64)], {
      type: "font/ttf",
    });
    return {
      kind: "font",
      packageId,
      tgi: resource.tgi,
      offset: 0,
      totalLength: data.totalLength,
      bytes: [],
      src: URL.createObjectURL(blob),
      totalBytes: data.totalLength,
    };
  }
  const bytes = await tauriApi.packages.readBytes(
    packageId,
    resource.tgi,
    0,
    Math.min(4096, resource.decompressedSize),
  );
  const base = {
    offset: bytes.offset,
    totalLength: bytes.totalLength,
    bytes: bytes.bytes,
  };
  if (resource.tgi.typeId === PROPERTY_TYPE_ID) {
    const data = await tauriApi.packages.readPropertyPreview(
      packageId,
      resource.tgi,
    );
    return { kind: "property", packageId, tgi: resource.tgi, ...data, ...base };
  }
  if (resource.tgi.typeId === RW4_TYPE_ID) {
    const data = await tauriApi.packages.readRw4Preview(packageId, resource.tgi);
    return {
      kind: "rw4",
      packageId,
      tgi: resource.tgi,
      ...data,
      ...base,
    };
  }
  const language = textPreviewLanguage(resource.tgi.typeId);
  if (language || looksLikeText(Uint8Array.from(bytes.bytes))) {
    // 全量字节经 ipc::Response 原始通道（4KB 仅用于嗅探，见 read_resource_text）
    const full = await tauriApi.packages.readResourceText(
      packageId,
      resource.tgi,
    );
    let payload = new Uint8Array(full);
    const truncated = payload.byteLength < resource.decompressedSize;
    if (language === "json") payload = stripLocaleJsonPrefix(payload);
    const { content, encoding } = decodeTextBytes(payload);
    return {
      kind: "text",
      ...base,
      content,
      encoding,
      language: language ?? "text",
      truncated,
    };
  }
  return { kind: "hex", ...base };
}

function svgPreviewUrl(instance: number) {
  const hue = instance % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360"><rect width="640" height="360" fill="hsl(${hue} 45% 18%)"/><circle cx="320" cy="170" r="92" fill="hsl(${(hue + 80) % 360} 80% 60%)"/><path d="M180 290h280" stroke="white" stroke-width="12" stroke-linecap="round" opacity=".75"/><text x="320" y="330" text-anchor="middle" fill="white" font-family="sans-serif" font-size="18">OpenSCP raster preview</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function mockResources() {
  return Array.from({ length: 2489 }, (_, index) => {
    const typeId = [
      0x2f4e681b,
      0x2f4e681c,
      0x00b1b104,
      0x0a98eaf0,
      0x0d9e5710,
      0x376840d7,
      0x0a4d8d09,
    ][index % 7];
    return {
      tgi: { typeId, group: index % 16, instance: 0x10000000 + index },
      offset: 1024 + index * 64,
      storedSize: 64,
      decompressedSize: typeId === 0x0a98eaf0 ? 128 : 4096,
      compressed: index % 3 === 0,
    };
  });
}

const mockTypeNames: Record<number, string> = {
  0x2f4e681b: "rw4",
  0x2f4e681c: "raster",
  0x00b1b104: "property",
  0x0a98eaf0: "text",
  0x0d9e5710: "wav audio",
  0x376840d7: "vp6 video",
  0x0a4d8d09: "Wwise SoundBank",
};

const mockPropertyEntries: PropertyResourceData["entries"] = [
  {
    hash: 0x0975695f,
    name: "Model Details",
    typeName: "Key",
    value: "T 2F4E681B - G 00000000 - I 10000001",
    arrayLen: null,
  },
  {
    hash: 0xcafe0002,
    name: "Alias Names",
    typeName: "string8",
    value: "city_hall, city_hall_lod0",
    arrayLen: 2,
  },
  {
    hash: 0xcafe0003,
    name: null,
    typeName: "float",
    value: "0.75",
    arrayLen: null,
  },
];

const mockRw4Sections: Rw4ResourceData["sections"] = [
  { number: 0, typeCode: 0x80005, typeName: "BBox", size: 56 },
  { number: 1, typeCode: 0x20009, typeName: "Mesh", size: 128 },
  { number: 2, typeCode: 0x20003, typeName: "Texture", size: 512 },
  { number: 3, typeCode: 0x10030, typeName: "Blob", size: 96 },
];

const MOCK_PNG_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

function mockTypeCounts(items: ResourceSummary[]): TypeCountInfo[] {
  const counts = new Map<number, number>();
  for (const item of items)
    counts.set(item.tgi.typeId, (counts.get(item.tgi.typeId) ?? 0) + 1);
  return [...counts.entries()]
    .sort(([a], [b]) => a - b)
    .map(([typeId, count]) => ({
      typeId,
      name: mockTypeNames[typeId] ?? typeId.toString(16).padStart(8, "0"),
      count,
    }));
}

function tgiText(tgi: Tgi) {
  return `${tgi.typeId.toString(16)}:${tgi.group.toString(16)}:${tgi.instance.toString(16)}`;
}
