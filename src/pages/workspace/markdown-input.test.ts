import { describe, expect, it } from "vitest";
import {
  continueList,
  indent,
  insideFence,
  insertCodeBlock,
  insertLink,
  insertTable,
  outdent,
  toggleHeading,
  toggleLineMarker,
  unclosedFenceStartLine,
  wrapInline,
  type TextState,
} from "./markdown-input";

/** "|" 标记折叠光标；"[...]" 标记选区；两者都没有时选区落在文末。 */
function state(input: string): TextState {
  const caret = input.indexOf("|");
  if (caret !== -1) {
    const text = input.replace("|", "");
    return { text, selectionStart: caret, selectionEnd: caret };
  }
  const rangeMatch = /\[([^[\]]*)\]/.exec(input);
  if (rangeMatch) {
    const text = input.replace("[", "").replace("]", "");
    const start = rangeMatch.index;
    return {
      text,
      selectionStart: start,
      selectionEnd: start + rangeMatch[1].length,
    };
  }
  return {
    text: input,
    selectionStart: input.length,
    selectionEnd: input.length,
  };
}

/** 全文选区（行级变换的常用输入）。 */
function selectAll(text: string): TextState {
  return { text, selectionStart: 0, selectionEnd: text.length };
}

describe("continueList", () => {
  it("continues a bullet list with the same marker", () => {
    const edit = continueList(state("- 已有项|"))!;
    expect(edit.text).toBe("- 已有项\n- ");
    expect(edit.selectionStart).toBe(8);
  });

  it("continues a task list with a reset checkbox", () => {
    const edit = continueList(state("- [x] 完成|"))!;
    expect(edit.text).toBe("- [x] 完成\n- [ ] ");
    expect(edit.selectionStart).toBe(edit.text.length);
  });

  it("increments ordered numbers", () => {
    const edit = continueList(state("3. 第三|"))!;
    expect(edit.text).toBe("3. 第三\n4. ");
    expect(edit.selectionStart).toBe(edit.text.length);
  });

  it("splits the line when the caret is mid-content", () => {
    const edit = continueList(state("- 前半|后半"))!;
    expect(edit.text).toBe("- 前半\n- 后半");
  });

  it("exits the list on an empty item", () => {
    const edit = continueList(state("- |"))!;
    expect(edit.text).toBe("");
    expect(edit.selectionStart).toBe(0);
  });

  it("keeps indentation when exiting a nested list", () => {
    const edit = continueList(state("- 父项\n  - |"))!;
    expect(edit.text).toBe("- 父项\n  ");
    expect(edit.selectionStart).toBe(7);
  });

  it("returns null outside lists (default newline)", () => {
    expect(continueList(state("普通段落|"))).toBeNull();
    expect(continueList(state("# 标题|"))).toBeNull();
  });

  it("does not hijack Enter when a range is selected", () => {
    expect(continueList(state("- 项[目]"))).toBeNull();
  });
});

describe("indent / outdent", () => {
  it("inserts two spaces at a collapsed caret", () => {
    const edit = indent(state("文字|"));
    expect(edit.text).toBe("文字  ");
    expect(edit.selectionStart).toBe(4);
  });

  it("indents every covered line and selects the lines", () => {
    const edit = indent(state("- a\n- [b]\n- c"));
    expect(edit.text).toBe("- a\n  - b\n- c");
    expect(edit.selectionStart).toBe(4);
    expect(edit.selectionEnd).toBe("  - b".length + 4);
  });

  it("outdents up to two leading spaces per line", () => {
    const edit = outdent(selectAll("  - a\n    - b"));
    expect(edit.text).toBe("- a\n  - b");
  });

  it("outdents a full tab stop", () => {
    const edit = outdent(state("\tcode|"));
    expect(edit.text).toBe("code");
  });
});

describe("wrapInline", () => {
  it("wraps the selection", () => {
    const edit = wrapInline(state("这是[重点]部分"), "**", "加粗文字");
    expect(edit.text).toBe("这是**重点**部分");
    expect(edit.text.slice(edit.selectionStart, edit.selectionEnd)).toBe(
      "重点",
    );
  });

  it("unwraps when already wrapped", () => {
    const edit = wrapInline(state("这是**[重点]**部分"), "**", "加粗文字");
    expect(edit.text).toBe("这是重点部分");
    expect(edit.text.slice(edit.selectionStart, edit.selectionEnd)).toBe(
      "重点",
    );
  });

  it("inserts a placeholder selected for typing", () => {
    const edit = wrapInline(state("前面|后面"), "**", "加粗文字");
    expect(edit.text).toBe("前面**加粗文字**后面");
    expect(edit.text.slice(edit.selectionStart, edit.selectionEnd)).toBe(
      "加粗文字",
    );
  });

  it("wraps with inline code backticks", () => {
    const edit = wrapInline(state("用[ctx]标记"), "`", "code");
    expect(edit.text).toBe("用`ctx`标记");
    expect(edit.text.slice(edit.selectionStart, edit.selectionEnd)).toBe("ctx");
  });
});

describe("toggleLineMarker", () => {
  it("adds a quote to every line", () => {
    const edit = toggleLineMarker(selectAll("第一行\n第二行"), "quote");
    expect(edit.text).toBe("> 第一行\n> 第二行");
  });

  it("removes markers when all lines already have them", () => {
    const edit = toggleLineMarker(selectAll("> 第一行\n> 第二行"), "quote");
    expect(edit.text).toBe("第一行\n第二行");
  });

  it("toggles a task checkbox without leaving residue", () => {
    let edit = toggleLineMarker(selectAll("待办事项"), "task");
    expect(edit.text).toBe("- [ ] 待办事项");
    edit = toggleLineMarker(selectAll(edit.text), "task");
    expect(edit.text).toBe("待办事项");
  });

  it("plain bullet toggle removes task checkbox too", () => {
    const edit = toggleLineMarker(selectAll("- [x] 已完成的事"), "bullet");
    expect(edit.text).toBe("已完成的事");
  });

  it("adds ordered list markers", () => {
    const edit = toggleLineMarker(selectAll("步骤一"), "ordered");
    expect(edit.text).toBe("1. 步骤一");
  });
});

describe("toggleHeading", () => {
  it("promotes a plain line to heading level", () => {
    expect(toggleHeading(state("标[题]"), 2).text).toBe("## 标题");
  });

  it("replaces an existing heading level", () => {
    expect(toggleHeading(state("# 标[题]"), 2).text).toBe("## 标题");
  });

  it("removes the heading when the level matches", () => {
    expect(toggleHeading(state("## 标[题]"), 2).text).toBe("标题");
  });

  it("handles all selected lines", () => {
    expect(toggleHeading(selectAll("甲\n乙"), 1).text).toBe("# 甲\n# 乙");
  });
});

describe("insertCodeBlock", () => {
  it("wraps the selection in fences", () => {
    const edit = insertCodeBlock(state("之前 [const a = 1;] 之后"));
    expect(edit.text).toBe("之前 \n```\nconst a = 1;\n```\n 之后");
    expect(edit.text.slice(edit.selectionStart, edit.selectionEnd)).toBe(
      "const a = 1;",
    );
  });

  it("inserts an empty block at line start with caret inside", () => {
    const edit = insertCodeBlock(state("|"));
    expect(edit.text).toBe("```\n\n```\n");
    expect(edit.selectionStart).toBe(4);
    expect(edit.selectionEnd).toBe(4);
  });

  it("prepends a newline when inserted mid-line", () => {
    const edit = insertCodeBlock(state("文字|"));
    expect(edit.text).toBe("文字\n```\n\n```\n");
  });

  it("unwraps the surrounding fence when the caret is inside one", () => {
    const edit = insertCodeBlock(state("```text\nconst a[ = 1;]\n```\n之后"));
    expect(edit.text).toBe("const a = 1;\n之后");
  });

  it("unwraps an unclosed fence above the caret", () => {
    const edit = insertCodeBlock(state("# 标题\n```\n草稿|"));
    expect(edit.text).toBe("# 标题\n草稿");
  });

  it("does nothing when the selection already spans a fence marker", () => {
    const text = "看下面\n```ts\nconst a = 1;\n```";
    const s: TextState = { text, selectionStart: 0, selectionEnd: text.length };
    expect(insertCodeBlock(s)).toBe(s);
  });
});

describe("unclosedFenceStartLine / insideFence", () => {
  it("returns -1 for balanced fences", () => {
    expect(unclosedFenceStartLine("```ts\ncode\n```\n正文")).toBe(-1);
  });

  it("reports the opening line of an unclosed fence", () => {
    expect(unclosedFenceStartLine("## 标题\n\n```text\n内容")).toBe(2);
  });

  it("only same-character markers of sufficient length close a fence", () => {
    expect(unclosedFenceStartLine("```\n~~~\n内容")).toBe(0);
    expect(unclosedFenceStartLine("~~~\n~~~\n内容")).toBe(-1);
    expect(unclosedFenceStartLine("````\n```\n内容")).toBe(0);
  });

  it("fence marker with trailing junk does not close", () => {
    expect(unclosedFenceStartLine("```\n``` text\n内容")).toBe(0);
  });

  it("insideFence tracks the caret position", () => {
    const text = "前文\n```ts\nconst a = 1;\n```\n后文";
    expect(insideFence(text, 2)).toBe(false);
    expect(insideFence(text, text.indexOf("```ts") + 6)).toBe(true);
    expect(insideFence(text, text.indexOf("const a") + 3)).toBe(true);
    expect(insideFence(text, text.indexOf("后文") + 1)).toBe(false);
  });

  it("continueList is inert inside a fence", () => {
    const inside = state("```\n- 看起来像列[表]\n```");
    expect(continueList(inside)).toBeNull();
    const outside = state("- 真列表|");
    expect(continueList(outside)).not.toBeNull();
  });
});

describe("insertLink / insertTable", () => {
  it("uses the selection as the link label and selects the URL", () => {
    const edit = insertLink(state("见[文档]说明"), "链接文字", "https://x.y");
    expect(edit.text).toBe("见[文档](https://x.y)说明");
    expect(edit.text.slice(edit.selectionStart, edit.selectionEnd)).toBe(
      "https://x.y",
    );
  });

  it("inserts a table skeleton and selects the first header cell", () => {
    const edit = insertTable(state("|"), ["列一", "列二", "列三"]);
    expect(edit.text).toBe(
      "| 列一 | 列二 | 列三 |\n| --- | --- | --- |\n|  |  |  |\n",
    );
    expect(edit.text.slice(edit.selectionStart, edit.selectionEnd)).toBe(
      "列一",
    );
  });
});
