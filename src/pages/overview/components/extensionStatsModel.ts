import type { ExtensionStat, PackageStat, PackageStatistics } from "@/api/tauri";

/** Lieflat palm SER — 色相=扩展名类目，跨卡固定。 */
export const PALM_SER = [
  "#8FBC6B",
  "#E8B84A",
  "#9AAF70",
  "#F2D17E",
  "#C5C48A",
  "#C4A484",
] as const;

export const UNKNOWN_COLOR = "var(--muted-foreground)";
export const SPARK_COLOR = "var(--info, #1280FE)";

export interface ExtSlice {
  ext: string;
  count: number;
  size: number;
  known: boolean;
}

export interface RankBar {
  key: string;
  label: string;
  value: number;
  percent: number;
  color: string;
}

export interface SparkPoint {
  label: string;
  value: number;
}

export function mergeExtensions(extensions: ExtensionStat[]): ExtSlice[] {
  const merged: ExtSlice[] = extensions
    .filter((item) => item.known)
    .map((item) => ({
      ext: item.ext,
      count: item.count,
      size: item.decompressedSize,
      known: true,
    }));
  const unknown = extensions.filter((item) => !item.known);
  if (unknown.length) {
    merged.push({
      ext: "unknown",
      count: unknown.reduce((total, item) => total + item.count, 0),
      size: unknown.reduce((total, item) => total + item.decompressedSize, 0),
      known: false,
    });
  }
  return merged;
}

export function colorOf(item: Pick<ExtSlice, "ext" | "known">): string {
  if (!item.known || item.ext === "unknown") return UNKNOWN_COLOR;
  let hash = 2166136261;
  for (let i = 0; i < item.ext.length; i += 1) {
    hash ^= item.ext.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return PALM_SER[hash % PALM_SER.length]!;
}

export function formatCompact(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0";
  if (value < 1000) return String(Math.round(value));
  if (value < 1_000_000) {
    const k = value / 1000;
    return `${k >= 100 ? k.toFixed(0) : k.toFixed(2).replace(/\.?0+$/, "")}k`;
  }
  const m = value / 1_000_000;
  return `${m >= 100 ? m.toFixed(0) : m.toFixed(2).replace(/\.?0+$/, "")}M`;
}

export function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = value;
  let unit = -1;
  do {
    size /= 1024;
    unit += 1;
  } while (size >= 1024 && unit < units.length - 1);
  return `${size.toFixed(size >= 100 ? 0 : 1)} ${units[unit]}`;
}

export function formatPercent(value: number, digits = 2): string {
  if (!Number.isFinite(value)) return "0%";
  return `${value.toFixed(digits)}%`;
}

/** 数量构成：按 count 排名的横向条（宽卡）。 */
export function countRankBars(slices: ExtSlice[], limit = 8): RankBar[] {
  const sorted = [...slices].sort((a, b) => b.count - a.count);
  const total = sorted.reduce((sum, item) => sum + item.count, 0) || 1;
  return sorted.slice(0, limit).map((item) => ({
    key: item.ext,
    label: item.ext,
    value: item.count,
    percent: (item.count / total) * 100,
    color: colorOf(item),
  }));
}

/** 体积主导类型：按 size 排名（与数量构成正交）。 */
export function sizeRankBars(slices: ExtSlice[], limit = 6): RankBar[] {
  const sorted = [...slices].sort((a, b) => b.size - a.size);
  const total = sorted.reduce((sum, item) => sum + item.size, 0) || 1;
  return sorted.slice(0, limit).map((item) => ({
    key: item.ext,
    label: item.ext,
    value: item.size,
    percent: (item.size / total) * 100,
    color: colorOf(item),
  }));
}

export function knownCoveragePercent(stats: PackageStatistics): number {
  const { knownDecompressed, decompressedSize } = stats.totals;
  if (!decompressedSize) return 0;
  return (knownDecompressed / decompressedSize) * 100;
}

/** stored / decompressed：越小表示压缩越狠；无 stored 时回退 fileSize。 */
export function compressionRatio(stats: PackageStatistics): number | null {
  const { storedSize, fileSize, decompressedSize } = stats.totals;
  const packed = storedSize > 0 ? storedSize : fileSize;
  if (!decompressedSize || !packed) return null;
  return packed / decompressedSize;
}

export function packageSparks(
  packages: PackageStat[],
  metric: "entryCount" | "decompressed",
): SparkPoint[] {
  const points = packages
    .map((pkg) => ({
      label: pkg.name,
      value:
        metric === "entryCount"
          ? pkg.entryCount
          : pkg.knownDecompressed + pkg.unknownDecompressed,
    }))
    .filter((point) => point.value > 0)
    .sort((a, b) => b.value - a.value)
    .slice(0, 12);
  return points.reverse();
}

/** Cloudflare 式 sparkline polyline（viewBox 0 0 100 36）。 */
export function sparklinePath(points: SparkPoint[]): string {
  if (!points.length) return "";
  const max = Math.max(...points.map((point) => point.value), 1);
  const step = points.length <= 1 ? 0 : 100 / (points.length - 1);
  return points
    .map((point, index) => {
      const x = points.length <= 1 ? 50 : index * step;
      const y = 34 - (point.value / max) * 30;
      return `${index === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");
}

/** 空态幽灵波形。 */
export function ghostSparkPath(): string {
  return "M 0 22 C 12 8, 20 30, 32 18 S 52 6, 64 20 S 84 32, 100 16";
}

export function topSizeExtension(slices: ExtSlice[]): ExtSlice | null {
  const sorted = [...slices].sort((a, b) => b.size - a.size);
  return sorted[0] ?? null;
}
