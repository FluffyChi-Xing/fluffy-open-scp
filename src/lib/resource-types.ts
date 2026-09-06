export type ResourceKind =
  "rw4" | "raster" | "property" | "text" | "media" | "other";

const extensionUrls = import.meta.glob("../assets/file-extensions/*.svg", {
  eager: true,
  import: "default",
  query: "?url",
}) as Record<string, string>;

const iconUrls = Object.fromEntries(
  Object.entries(extensionUrls).map(([path, url]) => [
    path.slice(path.lastIndexOf("/") + 1, -4).toLowerCase(),
    url,
  ]),
);

export const resourceKindMeta: Record<
  ResourceKind,
  { label: string; icon: string; fallback: string }
> = {
  rw4: { label: "package.rw4", icon: "rw4", fallback: "Box" },
  raster: { label: "package.raster", icon: "raster", fallback: "Image" },
  property: { label: "package.property", icon: "prop", fallback: "ListTree" },
  text: { label: "package.text", icon: "txt", fallback: "FileText" },
  media: { label: "package.media", icon: "mp4", fallback: "Film" },
  other: { label: "package.other", icon: "unknown", fallback: "FileQuestion" },
};

export function resourceKind(typeId: number): ResourceKind {
  if (typeId === 0x2f4e681b) return "rw4";
  if (typeId === 0x2f4e681c) return "raster";
  if (typeId === 0x00b1b104) return "property";
  if (typeId === 0x0d9e5710) return "media";
  if (typeId === 0x0a98eaf0) return "text";
  return "other";
}

const textPreviewLanguages: Record<number, string> = {
  0x67771f5c: "javascript",
  0x2c978db6: "css",
  0xdd6233d6: "html",
  0x0469a3f7: "cpp",
  0x0a98eaf0: "json",
};

export function textPreviewLanguage(typeId: number): string | null {
  return textPreviewLanguages[typeId] ?? null;
}

const imageMimes: Record<number, string> = {
  0x2f7d0004: "image/png",
  0x3f8662ea: "image/jpeg",
  0x2f7d0007: "image/gif",
};

export function imageMimeForType(typeId: number): string | null {
  return imageMimes[typeId] ?? null;
}

export function resourceIconUrl(kind: ResourceKind): string | undefined {
  return iconUrls[resourceKindMeta[kind].icon];
}

export function extensionIconUrl(fileName: string): string | undefined {
  const extension = fileName.split(".").pop()?.toLowerCase() ?? "unknown";
  return iconUrls[extension] ?? iconUrls.unknown;
}
