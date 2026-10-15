import { describe, expect, it } from "vitest";
import { diffLines } from "./lineDiff";

describe("diffLines", () => {
  it("相同文本返回全 same 行且行号逐行一致", () => {
    const text = "a\nb\nc";
    const rows = diffLines(text, text);
    expect(rows).toHaveLength(3);
    for (const [index, row] of rows.entries()) {
      expect(row.type).toBe("same");
      expect(row.oldNo).toBe(index + 1);
      expect(row.newNo).toBe(index + 1);
    }
  });

  it("纯追加：新增行带 newNo 无 oldNo", () => {
    const rows = diffLines("a\nb", "a\nX\nb");
    expect(rows).toEqual([
      { type: "same", text: "a", oldNo: 1, newNo: 1 },
      { type: "add", text: "X", newNo: 2 },
      { type: "same", text: "b", oldNo: 2, newNo: 3 },
    ]);
  });

  it("纯删除：删除行带 oldNo 无 newNo", () => {
    const rows = diffLines("a\nX\nb", "a\nb");
    expect(rows).toEqual([
      { type: "same", text: "a", oldNo: 1, newNo: 1 },
      { type: "del", text: "X", oldNo: 2 },
      { type: "same", text: "b", oldNo: 3, newNo: 2 },
    ]);
  });

  it("修改 = 删除 + 新增（JSON 值变化场景）", () => {
    const rows = diffLines('{"a": 1}', '{"a": 2}');
    const types = rows.map((row) => row.type);
    expect(types).toContain("del");
    expect(types).toContain("add");
    expect(types).not.toContain("same");
  });

  it("重建文本 = after 去掉 del 行 / = before 去掉 add 行", () => {
    const before = "unit 1\nkind: prop\nscale: 0.2\neditor: {}";
    const after = "unit 1\nkind: prop\nscale: 0.15\nrotation: 90\neditor: {}";
    const rows = diffLines(before, after);
    expect(
      rows
        .filter((row) => row.type !== "del")
        .map((row) => row.text)
        .join("\n"),
    ).toBe(after);
    expect(
      rows
        .filter((row) => row.type !== "add")
        .map((row) => row.text)
        .join("\n"),
    ).toBe(before);
    // 行号单调不减
    const oldNos = rows
      .map((row) => row.oldNo)
      .filter((value): value is number => value !== undefined);
    const newNos = rows
      .map((row) => row.newNo)
      .filter((value): value is number => value !== undefined);
    for (let index = 1; index < oldNos.length; index += 1) {
      expect(oldNos[index]).toBeGreaterThan(oldNos[index - 1]);
    }
    for (let index = 1; index < newNos.length; index += 1) {
      expect(newNos[index]).toBeGreaterThan(newNos[index - 1]);
    }
  });

  it("空串与文本互转：全删/全加", () => {
    expect(diffLines("", "a\nb")).toEqual([
      { type: "add", text: "a", newNo: 1 },
      { type: "add", text: "b", newNo: 2 },
    ]);
    expect(diffLines("a\nb", "")).toEqual([
      { type: "del", text: "a", oldNo: 1 },
      { type: "del", text: "b", oldNo: 2 },
    ]);
  });

  it("超限中段退化为整段替换但行号仍正确", () => {
    // 公共前后缀夹住超大中段（> 4e6 单元格），走 fallback 分支
    const head = "head\n";
    const tail = "\ntail";
    const big = (count: number, mark: string) =>
      Array.from({ length: count }, (_, i) => `${mark}${i}`).join("\n");
    const before = head + big(2100, "a") + tail;
    const after = head + big(2100, "b") + tail;
    const rows = diffLines(before, after);
    expect(rows[0]).toEqual({
      type: "same",
      text: "head",
      oldNo: 1,
      newNo: 1,
    });
    expect(rows.at(-1)).toEqual({
      type: "same",
      text: "tail",
      oldNo: 2102,
      newNo: 2102,
    });
    const mid = rows.slice(1, -1);
    expect(mid.every((row) => row.type === "del" || row.type === "add")).toBe(
      true,
    );
  });
});
