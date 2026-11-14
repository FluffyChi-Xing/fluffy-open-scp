import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * 架构守卫：计划层必须保持纯函数（无 three / DOM / 全局时间与随机）。
 * 这条测试是新基线的“门禁”，任何人在 plan/ 里 import three 都会失败。
 */
const PLAN_DIR = join(process.cwd(), "src/render/plan");
const files = readdirSync(PLAN_DIR).filter(
  (name) => name.endsWith(".ts") && !name.endsWith(".test.ts"),
);

const FORBIDDEN: [RegExp, string][] = [
  [/from\s+["']three["']/, "import three"],
  [/\bdocument\./, "document"],
  [/\bwindow\./, "window"],
  [/\bDate\.now\b/, "Date.now"],
  [/\bMath\.random\b/, "Math.random"],
  [/\bperformance\.now\b/, "performance.now"],
  [/\bnew Worker\(/, "Worker"],
  [/\blocalStorage\b/, "localStorage"],
];

describe("plan 层纯函数门禁", () => {
  it("plan/*.ts 不依赖 three / DOM / 全局时钟随机", () => {
    const violations: string[] = [];
    for (const file of files) {
      const source = readFileSync(join(PLAN_DIR, file), "utf8");
      for (const [pattern, label] of FORBIDDEN) {
        if (pattern.test(source)) violations.push(`${file}: ${label}`);
      }
    }
    expect(violations).toEqual([]);
  });

  it("plan 层不 import exec（单向依赖：plan ← exec）", () => {
    const violations = files.filter((file) =>
      /from\s+["'].*\/exec\//.test(readFileSync(join(PLAN_DIR, file), "utf8")),
    );
    expect(violations).toEqual([]);
  });
});
