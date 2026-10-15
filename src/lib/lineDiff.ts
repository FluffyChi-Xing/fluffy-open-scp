/**
 * 行级 unified diff（FCode diff 模式的底层）：LCS 回溯产出与 git diff
 * 同语义的行序列。性能策略 = 先修剪公共前缀/后缀（编辑通常局部化，剩余
 * 中段很小），中段超限时退化为整段删+整段加（正确性不变，仅可读性降级）。
 */

export type DiffRowType = "same" | "add" | "del";

export interface DiffRow {
  type: DiffRowType;
  /** 行文本（不含行尾符）。 */
  text: string;
  /** 旧文件 1 起行号（same/del 有值）。 */
  oldNo?: number;
  /** 新文件 1 起行号（same/add 有值）。 */
  newNo?: number;
}

/** LCS 中段单元格上限（n×m；超出即整段替换，防大文件 O(n·m) 爆内存）。 */
const LCS_CELL_CAP = 4_000_000;

export function diffLines(before: string, after: string): DiffRow[] {
  // 空串 = 空文件（split 会给 [""] 一个空行，产生多余的 del "" 行）
  const a = before ? before.split("\n") : [];
  const b = after ? after.split("\n") : [];
  const rows: DiffRow[] = [];
  let start = 0;
  const commonHead = Math.min(a.length, b.length);
  while (start < commonHead && a[start] === b[start]) start += 1;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA -= 1;
    endB -= 1;
  }
  for (let index = 0; index < start; index += 1) {
    rows.push({
      type: "same",
      text: a[index],
      oldNo: index + 1,
      newNo: index + 1,
    });
  }
  const midA = a.slice(start, endA);
  const midB = b.slice(start, endB);
  const n = midA.length;
  const m = midB.length;
  let oldNo = start + 1;
  let newNo = start + 1;
  if (n === 0 || m === 0 || n * m > LCS_CELL_CAP) {
    // 中段为空 / 超限：整段删 + 整段加（行号仍逐行正确）
    for (const text of midA) rows.push({ type: "del", text, oldNo: oldNo++ });
    for (const text of midB) rows.push({ type: "add", text, newNo: newNo++ });
  } else {
    const width = m + 1;
    const lcs = new Int32Array((n + 1) * width);
    for (let i = n - 1; i >= 0; i -= 1) {
      for (let j = m - 1; j >= 0; j -= 1) {
        lcs[i * width + j] =
          midA[i] === midB[j]
            ? lcs[(i + 1) * width + j + 1] + 1
            : Math.max(lcs[(i + 1) * width + j], lcs[i * width + j + 1]);
      }
    }
    let i = 0;
    let j = 0;
    while (i < n && j < m) {
      if (midA[i] === midB[j]) {
        rows.push({
          type: "same",
          text: midA[i],
          oldNo: oldNo++,
          newNo: newNo++,
        });
        i += 1;
        j += 1;
      } else if (lcs[(i + 1) * width + j] >= lcs[i * width + j + 1]) {
        rows.push({ type: "del", text: midA[i], oldNo: oldNo++ });
        i += 1;
      } else {
        rows.push({ type: "add", text: midB[j], newNo: newNo++ });
        j += 1;
      }
    }
    while (i < n) rows.push({ type: "del", text: midA[i++], oldNo: oldNo++ });
    while (j < m) rows.push({ type: "add", text: midB[j++], newNo: newNo++ });
  }
  for (let index = 0; index < a.length - endA; index += 1) {
    rows.push({
      type: "same",
      text: a[endA + index],
      oldNo: endA + index + 1,
      newNo: endB + index + 1,
    });
  }
  return rows;
}
