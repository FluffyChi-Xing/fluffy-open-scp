import { describe, expect, it } from "vitest";
import {
  classifyCodeFile,
  extensionOf,
  flattenCodeTree,
  formatCodeSize,
  joinCodePath,
  type CodeTreeNodeDto,
} from "./code-support";

describe("code file classification", () => {
  it("routes extensions to viewer kinds", () => {
    expect(classifyCodeFile("mod.package")).toBe("package");
    expect(classifyCodeFile("描述.PNG")).toBe("image");
    expect(classifyCodeFile("readme.txt")).toBe("text");
    expect(classifyCodeFile("setup.bat")).toBe("text");
    expect(classifyCodeFile("script.lua")).toBe("text");
    expect(classifyCodeFile("config.json")).toBe("text");
    expect(classifyCodeFile("asset.bin")).toBe("binary");
    expect(classifyCodeFile("noext")).toBe("binary");
  });

  it("extracts lowercase extensions without dotfile false positives", () => {
    expect(extensionOf("a.b.Png")).toBe("png");
    expect(extensionOf(".gitignore")).toBe("");
    expect(extensionOf("name.")).toBe("");
    expect(extensionOf("name")).toBe("");
  });
});

describe("formatCodeSize", () => {
  it("scales units readably", () => {
    expect(formatCodeSize(512)).toBe("512 B");
    expect(formatCodeSize(2048)).toBe("2.0 KB");
    expect(formatCodeSize(5 * 1024 * 1024)).toBe("5.0 MB");
  });
});

describe("joinCodePath", () => {
  it("joins with backslashes for verbatim roots", () => {
    expect(joinCodePath("\\\\?\\D:\\open-scp-mods", "Mod/a.package")).toBe(
      "\\\\?\\D:\\open-scp-mods\\Mod\\a.package",
    );
    expect(joinCodePath("D:\\mods\\", "readme.txt")).toBe(
      "D:\\mods\\readme.txt",
    );
    expect(joinCodePath("D:\\mods", "")).toBe("D:\\mods");
  });
});

describe("flattenCodeTree", () => {
  const tree: CodeTreeNodeDto[] = [
    {
      name: "Mod",
      relativePath: "Mod",
      kind: "folder",
      size: null,
      children: [
        {
          name: "data",
          relativePath: "Mod/data",
          kind: "folder",
          size: null,
          children: [],
        },
        {
          name: "readme.txt",
          relativePath: "Mod/readme.txt",
          kind: "file",
          size: 5,
          children: [],
        },
      ],
    },
  ];

  it("stays collapsed without expanded folders", () => {
    const rows = flattenCodeTree(tree, new Set());
    expect(rows.map((row) => row.relativePath)).toEqual(["Mod"]);
  });

  it("walks expanded folders depth-first with depth info", () => {
    const rows = flattenCodeTree(tree, new Set(["Mod"]));
    expect(rows.map((row) => row.relativePath)).toEqual([
      "Mod",
      "Mod/data",
      "Mod/readme.txt",
    ]);
    expect(rows.map((row) => row.depth)).toEqual([0, 1, 1]);
  });
});
