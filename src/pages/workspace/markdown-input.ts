/**
 * Markdown 录入辅助（纯函数）：输入当前文本 + 选区，输出新文本 + 新选区。
 * 编辑器组件负责把这些变换套到 textarea 上（更新值并恢复选区）。
 * 全部函数不得依赖 DOM 或组件状态，便于单元测试。
 */

export interface TextState {
  text: string;
  selectionStart: number;
  selectionEnd: number;
}

export type TextEdit = TextState;

const INDENT = "  ";
const FENCE = "```";

const FENCE_OPEN_RE = /^(\s*)(`{3,}|~{3,})/;
const FENCE_CLOSE_RE = /^\s*(`{3,}|~{3,})\s*$/;

/** 打开中的代码围栏：起始行号、围栏字符（` 或 ~）与最小闭合长度。 */
interface FenceState {
  openAt: number;
  char: string;
  minLen: number;
}

/** 逐行跟踪围栏开合，返回 upTo 行（含）之后仍打开的围栏；无则 null。 */
function fenceStateAtLine(lines: string[], upTo: number): FenceState | null {
  let state: FenceState | null = null;
  for (let i = 0; i <= upTo && i < lines.length; i++) {
    const close = FENCE_CLOSE_RE.exec(lines[i]);
    if (state) {
      if (
        close &&
        close[1][0] === state.char &&
        close[1].length >= state.minLen
      ) {
        state = null;
      }
    } else {
      const open = FENCE_OPEN_RE.exec(lines[i]);
      if (open) state = { openAt: i, char: open[2][0], minLen: open[2].length };
    }
  }
  return state;
}

/** 光标位置是否处于打开的代码围栏内部（围栏内不做列表续行等智能按键）。 */
export function insideFence(text: string, position: number): boolean {
  const lines = text.split("\n");
  const line = text.slice(0, position).split("\n").length - 1;
  return fenceStateAtLine(lines, line) !== null;
}

/**
 * 文档中第一个未闭合围栏的起始行号（0 基）；全部闭合返回 -1。
 * 未闭合围栏会让预览把其后全部内容当作代码，编辑器据此提示。
 */
export function unclosedFenceStartLine(text: string): number {
  const state = fenceStateAtLine(text.split("\n"), Number.MAX_SAFE_INTEGER);
  return state ? state.openAt : -1;
}

/** 光标所在（或选区覆盖）行的 [start, end) 边界。 */
function lineBounds(text: string, position: number): [number, number] {
  const start = text.lastIndexOf("\n", position - 1) + 1;
  let end = text.indexOf("\n", position);
  if (end === -1) end = text.length;
  return [start, end];
}

function lineRange(state: TextState): {
  start: number;
  end: number;
  lines: string[];
} {
  const [firstStart] = lineBounds(state.text, state.selectionStart);
  const [, lastEnd] = lineBounds(state.text, state.selectionEnd);
  const lines = state.text.slice(firstStart, lastEnd).split("\n");
  return { start: firstStart, end: lastEnd, lines };
}

/**
 * Enter 键的列表续行：
 * - 当前行是无序/有序/任务列表项且光标前有内容 → 换行并补同类标记
 *   （有序列表序号自增，任务列表重置为未勾选）；
 * - 当前行是空列表项（标记后无内容）→ 清除标记退出列表；
 * - 不在列表中 → 返回 null，走浏览器默认换行。
 * 光标在行中间时：光标后的内容随新标记移到下一行（标准编辑器行为）。
 */
export function continueList(state: TextState): TextEdit | null {
  if (state.selectionStart !== state.selectionEnd) return null;
  const position = state.selectionStart;
  if (insideFence(state.text, position)) return null;
  const [lineStart, lineEnd] = lineBounds(state.text, position);
  const line = state.text.slice(lineStart, lineEnd);
  // 只看光标之前的行首部分，行尾内容不影响当前项判断
  const before = line.slice(0, position - lineStart);
  const task = /^(\s*)([-*+])\s\[( |x|X)\]\s/.exec(before);
  const bullet = /^(\s*)([-*+])\s/.exec(before);
  const ordered = /^(\s*)(\d{1,9})([.)])\s/.exec(before);
  let marker: string;
  if (task) marker = `${task[1]}${task[2]} [ ] `;
  else if (ordered)
    marker = `${ordered[1]}${Number(ordered[2]) + 1}${ordered[3]} `;
  else if (bullet) marker = `${bullet[1]}${bullet[2]} `;
  else return null;
  if (before.slice(marker.length).trim() === "") {
    // 空项：清掉标记退出列表（保留缩进）
    const indent = (task ?? ordered ?? bullet)![1];
    return {
      text: state.text.slice(0, lineStart) + indent + state.text.slice(lineEnd),
      selectionStart: lineStart + indent.length,
      selectionEnd: lineStart + indent.length,
    };
  }
  return {
    text:
      state.text.slice(0, position) +
      `\n${marker}` +
      state.text.slice(position),
    selectionStart: position + 1 + marker.length,
    selectionEnd: position + 1 + marker.length,
  };
}

/** Tab 缩进：折叠光标时插入两个空格；跨行选区整体缩进。 */
export function indent(state: TextState): TextEdit {
  if (state.selectionStart === state.selectionEnd) {
    const position = state.selectionStart;
    return {
      text: state.text.slice(0, position) + INDENT + state.text.slice(position),
      selectionStart: position + INDENT.length,
      selectionEnd: position + INDENT.length,
    };
  }
  return shiftLines(state, INDENT.length);
}

/** Shift+Tab 反缩进：每行去掉至多一个制表符或两个前导空格。 */
export function outdent(state: TextState): TextEdit {
  return shiftLines(state, -INDENT.length);
}

/** 行级缩进/反缩进，变换后选区覆盖全部受影响行（便于连续按 Tab）。 */
function shiftLines(state: TextState, amount: number): TextEdit {
  const range = lineRange(state);
  const shifted = range.lines.map((line) => {
    if (amount >= 0) return INDENT + line;
    const leading = /^ */.exec(line)![0].length;
    const strip = line.startsWith("\t") ? 1 : Math.min(INDENT.length, leading);
    return line.slice(strip);
  });
  const joined = shifted.join("\n");
  const text =
    state.text.slice(0, range.start) + joined + state.text.slice(range.end);
  return {
    text,
    selectionStart: range.start,
    selectionEnd: range.start + joined.length,
  };
}

/**
 * 行内包裹（加粗/斜体/行内代码）：选区两侧已有相同包裹 → 解开；否则包上。
 * 选区为空时插入包裹并选中占位文本，方便直接输入。
 */
export function wrapInline(
  state: TextState,
  wrap: string,
  placeholder: string,
): TextEdit {
  const { text, selectionStart: start, selectionEnd: end } = state;
  const selected = text.slice(start, end);
  if (
    start >= wrap.length &&
    text.slice(start - wrap.length, start) === wrap &&
    text.slice(end, end + wrap.length) === wrap
  ) {
    const unwrapStart = start - wrap.length;
    return {
      text:
        text.slice(0, unwrapStart) + selected + text.slice(end + wrap.length),
      selectionStart: unwrapStart,
      selectionEnd: unwrapStart + selected.length,
    };
  }
  if (selected) {
    return {
      text: text.slice(0, start) + wrap + selected + wrap + text.slice(end),
      selectionStart: start + wrap.length,
      selectionEnd: end + wrap.length,
    };
  }
  return {
    text: text.slice(0, start) + wrap + placeholder + wrap + text.slice(start),
    selectionStart: start + wrap.length,
    selectionEnd: start + wrap.length + placeholder.length,
  };
}

/** 行首标记切换的识别正则与新增前缀（引用/列表/任务）。 */
const LINE_MARKERS = {
  quote: { detect: /^(\s*)>\s?/, prefix: "> " },
  // 无序列表的识别同时吃掉任务勾选框，避免「取消列表」残留 [ ]
  bullet: { detect: /^(\s*)[-*+]\s+(?:\[(?: |x|X)\]\s+)?/, prefix: "- " },
  ordered: { detect: /^(\s*)\d{1,9}[.)]\s+/, prefix: "1. " },
  task: { detect: /^(\s*)([-*+])\s+\[(?: |x|X)\]\s+/, prefix: "- [ ] " },
} as const;

export type LineMarkerKind = keyof typeof LINE_MARKERS;

/**
 * 行级标记切换（引用/无序/有序/任务）：选区内所有行都已带标记 → 全部
 * 移除；否则全部加上。标题级别互相替换，走 toggleHeading。
 */
export function toggleLineMarker(
  state: TextState,
  kind: LineMarkerKind,
): TextEdit {
  const { detect, prefix } = LINE_MARKERS[kind];
  const range = lineRange(state);
  const allMarked = range.lines.every((line) => detect.test(line));
  const stripped = range.lines.map((line) => line.replace(detect, "$1"));
  const lines = allMarked ? stripped : stripped.map((line) => prefix + line);
  const joined = lines.join("\n");
  const text =
    state.text.slice(0, range.start) + joined + state.text.slice(range.end);
  return {
    text,
    selectionStart: range.start,
    selectionEnd: range.start + joined.length,
  };
}

/** 标题切换：同一级别再点一次移除标题；不同级别直接替换。 */
export function toggleHeading(
  state: TextState,
  level: 1 | 2 | 3 | 4 | 5 | 6,
): TextEdit {
  const range = lineRange(state);
  const target = `${"#".repeat(level)} `;
  const lines = range.lines.map((line) => {
    const bare = line.replace(/^#{1,6}\s+/, "");
    const existing = /^#{1,6}\s+/.exec(line)?.[0];
    return existing === target ? bare : target + bare;
  });
  const joined = lines.join("\n");
  const text =
    state.text.slice(0, range.start) + joined + state.text.slice(range.end);
  return {
    text,
    selectionStart: range.start,
    selectionEnd: range.start + joined.length,
  };
}

/**
 * 插入围栏代码块：包住选区；光标折叠时插入空块并把光标落在其中。
 * 光标已处于围栏内 → 改为移除包围它的围栏（切换语义，杜绝嵌套双围栏，
 * 嵌套会让预览从内层标记行起全部当代码渲染）；选区横跨围栏标记行时
 * 不动作，避免产生早期闭合的坏围栏。
 */
export function insertCodeBlock(state: TextState): TextEdit {
  const { text, selectionStart: start, selectionEnd: end } = state;
  const lines = text.split("\n");
  const caretLine = text.slice(0, start).split("\n").length - 1;
  const open = fenceStateAtLine(lines, caretLine);
  if (open) return unwrapFence(lines, open);
  const selected = text.slice(start, end);
  if (/^\s*(`{3,}|~{3,})/m.test(selected)) return state;
  const lead = start === 0 || text[start - 1] === "\n" ? "" : "\n";
  const insert = `${lead}${FENCE}\n${selected}\n${FENCE}\n`;
  const bodyStart = start + lead.length + FENCE.length + 1;
  return {
    text: text.slice(0, start) + insert + text.slice(end),
    selectionStart: bodyStart,
    selectionEnd: bodyStart + selected.length,
  };
}

/** 移除打开围栏的开头行与闭合行（未闭合时只移除开头行），光标落在原开头处。 */
function unwrapFence(lines: string[], fence: FenceState): TextEdit {
  let closeLine = -1;
  for (let i = fence.openAt + 1; i < lines.length; i++) {
    const close = FENCE_CLOSE_RE.exec(lines[i]);
    if (
      close &&
      close[1][0] === fence.char &&
      close[1].length >= fence.minLen
    ) {
      closeLine = i;
      break;
    }
  }
  const text = lines
    .filter((_, i) => i !== fence.openAt && i !== closeLine)
    .join("\n");
  const caret =
    lines.slice(0, fence.openAt).join("\n").length + (fence.openAt > 0 ? 1 : 0);
  return { text, selectionStart: caret, selectionEnd: caret };
}

/** 插入链接：选区作为链接文字；插入后选中 URL 便于直接粘贴。 */
export function insertLink(
  state: TextState,
  placeholderText: string,
  placeholderUrl: string,
): TextEdit {
  const { text, selectionStart: start, selectionEnd: end } = state;
  const label = text.slice(start, end) || placeholderText;
  const insert = `[${label}](${placeholderUrl})`;
  const urlStart = start + label.length + 3;
  return {
    text: text.slice(0, start) + insert + text.slice(end),
    selectionStart: urlStart,
    selectionEnd: urlStart + placeholderUrl.length,
  };
}

/** 在光标处插入三列表格骨架，选区落在表头第一格。 */
export function insertTable(
  state: TextState,
  headerPlaceholders: [string, string, string],
): TextEdit {
  const { text, selectionStart: start, selectionEnd: end } = state;
  const [a, b, c] = headerPlaceholders;
  const table = `| ${a} | ${b} | ${c} |\n| --- | --- | --- |\n|  |  |  |\n`;
  const lead = start === 0 || text[start - 1] === "\n" ? "" : "\n";
  const insert = lead + table;
  const headStart = start + lead.length + 2;
  return {
    text: text.slice(0, start) + insert + text.slice(end),
    selectionStart: headStart,
    selectionEnd: headStart + a.length,
  };
}
