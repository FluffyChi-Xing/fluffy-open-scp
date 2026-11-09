import { describe, expect, it } from "vitest";
import { buildLanguageSegments, formatPercent } from "./languageBar";

describe("buildLanguageSegments", () => {
  it("ranks by bytes and folds the tail into other", () => {
    const languages = Array.from({ length: 7 }, (_, index) => ({
      extension: `e${index}`,
      fileCount: 1,
      bytes: 700 - index * 100,
    }));
    const segments = buildLanguageSegments(languages, 2800, "Other");
    expect(segments).toHaveLength(6);
    expect(segments[0]?.extension).toBe("e0");
    expect(segments.at(-1)?.extension).toBe("other");
    expect(segments.at(-1)?.label).toBe("Other");
  });
});

describe("formatPercent", () => {
  it("formats tiny and normal ratios", () => {
    expect(formatPercent(0)).toBe("0%");
    expect(formatPercent(0.004)).toBe("<1%");
    expect(formatPercent(0.559)).toBe("55.9%");
  });
});
