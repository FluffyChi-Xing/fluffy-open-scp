/**
 * Code 工作台（M-CM1/M-CM2）的文件分类与展示辅助：
 * 按扩展名把文件分流到查看器（文本 / 图片 / DBPF 容器 / 二进制），
 * 与后端 code_read_text / read_image_rgba / code_package_info 一一对应。
 */

export type CodeViewerKind = "text" | "image" | "package" | "binary";

const TEXT_EXTENSIONS = new Set([
  "txt",
  "md",
  "markdown",
  "json",
  "xml",
  "js",
  "mjs",
  "cjs",
  "ts",
  "lua",
  "bat",
  "cmd",
  "ps1",
  "cfg",
  "ini",
  "yaml",
  "yml",
  "css",
  "html",
  "htm",
  "csv",
  "tsv",
  "prop",
  "twee",
  "py",
  "sh",
  "log",
  "toml",
]);

const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "webp", "gif", "bmp"]);

/** 小写扩展名；无扩展名或点开头（.gitignore 等）返回空串。 */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return "";
  return name.slice(dot + 1).toLowerCase();
}

export function classifyCodeFile(name: string): CodeViewerKind {
  const extension = extensionOf(name);
  if (extension === "package") return "package";
  if (IMAGE_EXTENSIONS.has(extension)) return "image";
  if (TEXT_EXTENSIONS.has(extension)) return "text";
  return "binary";
}

/** shiki 语言映射（FCode 高亮用）。txt 等纯文本回退 "text" = 不高亮。 */
const SHIKI_LANGUAGE_BY_EXTENSION: Record<string, string> = {
  bat: "bat",
  cmd: "bat",
  ps1: "powershell",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  ts: "typescript",
  json: "json",
  xml: "xml",
  html: "html",
  htm: "html",
  css: "css",
  md: "markdown",
  markdown: "markdown",
  lua: "lua",
  py: "python",
  sh: "shellscript",
  yaml: "yaml",
  yml: "yaml",
  toml: "toml",
  ini: "ini",
};

export function shikiLanguageOf(name: string): string {
  return SHIKI_LANGUAGE_BY_EXTENSION[extensionOf(name)] ?? "text";
}

export function formatCodeSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/** modRoot 由后端 canonicalize（Windows 下带 \\?\ verbatim 前缀），
 *  该形态不接受 / 分隔——拼接必须用反斜杠。 */
export function joinCodePath(root: string, relativePath: string): string {
  if (!relativePath) return root;
  const separator = root.endsWith("\\") ? "" : "\\";
  return `${root}${separator}${relativePath.replaceAll("/", "\\")}`;
}

export interface CodeTreeNodeDto {
  name: string;
  relativePath: string;
  kind: "folder" | "file";
  size: number | null;
  children: CodeTreeNodeDto[];
}

export interface CodeRow {
  name: string;
  relativePath: string;
  kind: "folder" | "file";
  size: number | null;
  depth: number;
}

/** 树 → 可见行（依赖 expanded 集合下钻；顺序即渲染顺序）。 */
export function flattenCodeTree(
  entries: CodeTreeNodeDto[],
  expanded: Set<string>,
): CodeRow[] {
  const rows: CodeRow[] = [];
  const walk = (nodes: CodeTreeNodeDto[], depth: number) => {
    for (const node of nodes) {
      rows.push({
        name: node.name,
        relativePath: node.relativePath,
        kind: node.kind,
        size: node.size,
        depth,
      });
      if (node.kind === "folder" && expanded.has(node.relativePath)) {
        walk(node.children, depth + 1);
      }
    }
  };
  walk(entries, 0);
  return rows;
}
