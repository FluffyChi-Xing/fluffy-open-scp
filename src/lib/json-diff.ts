/**
 * 行级 JSON diff：两份 pretty-print 文本按行做 LCS 对齐，
 * 输出统一 diff 风格的行（" " 上下文 / "-" 基线独有 / "+" 当前独有）。
 * schema 文档规模在数百行级，O(n·m) DP 足够。
 */
export interface DiffLine {
  kind: " " | "-" | "+";
  text: string;
}

export function diffLines(a: string[], b: string[]): DiffLine[] {
  const n = a.length;
  const m = b.length;
  // lcs[i][j] = a[i..] 与 b[j..] 的最长公共子序列长度
  const lcs: Uint32Array[] = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      out.push({ kind: " ", text: a[i] });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      out.push({ kind: "-", text: a[i] });
      i++;
    } else {
      out.push({ kind: "+", text: b[j] });
      j++;
    }
  }
  while (i < n) out.push({ kind: "-", text: a[i++] });
  while (j < m) out.push({ kind: "+", text: b[j++] });
  return out;
}

export function formatDiff(lines: DiffLine[]): string {
  return lines.map((line) => (line.kind === " " ? `  ${line.text}` : `${line.kind} ${line.text}`)).join("\n");
}
