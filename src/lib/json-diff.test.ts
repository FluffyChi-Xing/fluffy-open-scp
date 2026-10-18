import { describe, expect, it } from "vitest";
import { diffLines, formatDiff } from "./json-diff";

describe("diffLines", () => {
  it("新增行以 + 标出，其余为上下文", () => {
    const a = ["{", '  "a": 1', "}"];
    const b = ["{", '  "a": 1', '  "b": 2', "}"];
    const lines = diffLines(a, b);
    expect(formatDiff(lines)).toBe(['  {', '    "a": 1', '+   "b": 2', '  }'].join("\n"));
  });

  it("删除行以 - 标出", () => {
    const a = ["{", '  "gone": true', "}"];
    const b = ["{", "}"];
    const lines = diffLines(a, b);
    expect(formatDiff(lines)).toBe(['  {', '-   "gone": true', '  }'].join("\n"));
  });

  it("完全一致时只输出上下文", () => {
    const a = ["x", "y"];
    expect(formatDiff(diffLines(a, [...a]))).toBe("  x\n  y");
  });
});
