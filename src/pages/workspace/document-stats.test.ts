import { describe, expect, it } from "vitest";
import { countCharacters, countWords, documentStats } from "./document-stats";

describe("countWords", () => {
  it("counts latin word runs as one word", () => {
    expect(countWords("hello world foo_bar")).toBe(3);
    expect(countWords("well-known CJK-friendly v2")).toBe(3);
  });

  it("counts each CJK character as one word", () => {
    expect(countWords("文档工作区")).toBe(5);
    expect(countWords("こんにちは")).toBe(5);
  });

  it("counts mixed text with both rules", () => {
    // "文档" 2 词 + "rendering" 1 词 + "管线" 2 词
    expect(countWords("文档 rendering 管线")).toBe(5);
  });

  it("treats korean runs as single words (space separated)", () => {
    expect(countWords("안녕하세요 world")).toBe(2);
  });

  it("ignores punctuation and whitespace", () => {
    expect(countWords("  # 标题\n\n- item **bold**...")).toBe(4);
  });

  it("returns zero for empty text", () => {
    expect(countWords("")).toBe(0);
    expect(countWords("  \n\t ")).toBe(0);
  });
});

describe("countCharacters", () => {
  it("excludes whitespace", () => {
    expect(countCharacters("a b\nc\td")).toBe(4);
    expect(countCharacters("文档 工作区")).toBe(5);
  });

  it("counts punctuation", () => {
    expect(countCharacters("a, b。")).toBe(4);
  });
});

describe("documentStats", () => {
  it("returns both counts", () => {
    expect(documentStats("hello 世界")).toEqual({ words: 3, characters: 7 });
  });
});
