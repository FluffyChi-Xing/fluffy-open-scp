import { isTauri, tauriApi } from "./index";
import type {
  GameFolder,
  OpenPackageResponse,
  PackageFile,
  PackageHistory,
  ResourceBytes,
  ResourcePage,
  ResourcePreview,
  ResourceSummary,
  ResolvedResourceName,
  Tgi,
  WorkspaceEntry,
  WorkspaceStatus,
  TypeCountInfo,
} from "./tauri";
import { mockOverview } from "./mock-data";
import { imageMimeForType, textPreviewLanguage } from "@/lib/resource-types";

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
  closePackage(packageId: number): Promise<void>;
  activityPackages(): Promise<PackageHistory[]>;
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
    previewResource: tauriPreview,
    closePackage: tauriApi.packages.close,
    activityPackages: tauriApi.activity.packages,
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
      if (imageMimeForType(resource.tgi.typeId) || resource.tgi.typeId === 0x2f4e681c)
        return {
          kind: "image",
          ...base,
          src: svgPreviewUrl(resource.tgi.instance),
          mime: "image/svg+xml",
          width: 320,
          height: 180,
        };
      return { kind: "hex", ...base };
    },
    async closePackage() {},
    async activityPackages() {
      return mockOverview.recentPackages;
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

async function tauriPreview(
  packageId: number,
  resource: ResourceSummary,
): Promise<ResourcePreview> {
  const mime = imageMimeForType(resource.tgi.typeId);
  if (mime) {
    const data = await tauriApi.packages.readData(packageId, resource.tgi);
    const blob = new Blob([base64ToBytes(data.dataBase64)], { type: mime });
    return {
      kind: "image",
      offset: 0,
      totalLength: data.totalLength,
      bytes: [],
      src: URL.createObjectURL(blob),
      mime,
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
  const language = textPreviewLanguage(resource.tgi.typeId);
  if (language || isTextBytes(bytes.bytes)) {
    const content = new TextDecoder("utf-8", { fatal: false }).decode(
      Uint8Array.from(bytes.bytes),
    );
    return {
      kind: "text",
      ...base,
      content,
      encoding: "utf-8",
      language: language ?? "text",
      truncated: bytes.totalLength > bytes.bytes.length,
    };
  }
  return { kind: "hex", ...base };
}

function isTextBytes(bytes: number[]) {
  return (
    bytes.length > 0 &&
    bytes.every(
      (byte) =>
        byte === 9 || byte === 10 || byte === 13 || (byte >= 32 && byte <= 126),
    )
  );
}

function svgPreviewUrl(instance: number) {
  const hue = instance % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360"><rect width="640" height="360" fill="hsl(${hue} 45% 18%)"/><circle cx="320" cy="170" r="92" fill="hsl(${(hue + 80) % 360} 80% 60%)"/><path d="M180 290h280" stroke="white" stroke-width="12" stroke-linecap="round" opacity=".75"/><text x="320" y="330" text-anchor="middle" fill="white" font-family="sans-serif" font-size="18">OpenSCP raster preview</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function mockResources() {
  return Array.from({ length: 2489 }, (_, index) => {
    const typeId = [0x2f4e681b, 0x2f4e681c, 0x00b1b104, 0x0a98eaf0, 0x0d9e5710][
      index % 5
    ];
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
};

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
