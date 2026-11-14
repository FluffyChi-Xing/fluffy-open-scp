import type { ModProjectLanguageStat } from "@/api/tauri";

/**
 * Languages 条：按字节占比切分色段（GitHub 风格 / lieflat Mono 明度阶）。
 * 扩展名 → 稳定色，避免每次刷新跳色。
 */

const PALETTE = [
  "var(--primary)",
  "color-mix(in srgb, var(--primary) 72%, var(--foreground))",
  "color-mix(in srgb, var(--primary) 48%, var(--muted-foreground))",
  "color-mix(in srgb, var(--chart-2, #3dd68c) 80%, transparent)",
  "color-mix(in srgb, var(--chart-3, #f0b429) 80%, transparent)",
  "color-mix(in srgb, var(--chart-4, #a78bfa) 75%, transparent)",
  "color-mix(in srgb, var(--muted-foreground) 55%, transparent)",
] as const;

export type LanguageSegment = {
  extension: string;
  label: string;
  fileCount: number;
  bytes: number;
  ratio: number;
  color: string;
};

function colorFor(extension: string, index: number): string {
  let hash = 0;
  for (let i = 0; i < extension.length; i += 1) {
    hash = (hash * 31 + extension.charCodeAt(i)) >>> 0;
  }
  return PALETTE[(hash + index) % PALETTE.length] ?? PALETTE[0];
}

export function buildLanguageSegments(
  languages: ModProjectLanguageStat[],
  totalBytes: number,
  otherLabel: string,
): LanguageSegment[] {
  if (!languages.length || totalBytes <= 0) return [];
  const top = languages.slice(0, 5);
  const rest = languages.slice(5);
  const segments: LanguageSegment[] = top.map((entry, index) => ({
    extension: entry.extension,
    label: entry.extension === "(none)" ? otherLabel : entry.extension,
    fileCount: entry.fileCount,
    bytes: entry.bytes,
    ratio: entry.bytes / totalBytes,
    color: colorFor(entry.extension, index),
  }));
  if (rest.length) {
    const bytes = rest.reduce((sum, entry) => sum + entry.bytes, 0);
    const fileCount = rest.reduce((sum, entry) => sum + entry.fileCount, 0);
    segments.push({
      extension: "other",
      label: otherLabel,
      fileCount,
      bytes,
      ratio: bytes / totalBytes,
      color: PALETTE[PALETTE.length - 1]!,
    });
  }
  return segments;
}

export function formatPercent(ratio: number): string {
  if (ratio <= 0) return "0%";
  if (ratio < 0.01) return "<1%";
  return `${(ratio * 100).toFixed(1)}%`;
}
