/**
 * 游戏 UI 重建装配方法：用屏幕定义 + locale 字符串表 + 静态图片 + 游戏 CSS
 * 生成一份可直接挂载到 iframe srcdoc 的完整 HTML 文档。
 *
 * 数据三源（全部来自解包资产，见 docs/overview/ui-assets.md）：
 * - public/game-ui/css/*.css —— 游戏原生组件样式（真实 <link> 引入）
 * - public/game-ui/locale/<FNV-1>.json —— 该屏幕的真实 UI 文案
 * - public/game-ui/png/* —— 采样图片
 *
 * 装配是纯函数：`assembleGameUiHtml(spec, strings)` 不依赖 Vue 运行时，
 * 后续 WP3 编辑器保存的 JSON 也可以直接走同一条装配管线预览。
 */

export interface ScreenSpec {
  id: string;
  screen: string;
  group: string;
  localePath: string | null;
  images: string[];
}

/** 游戏 CSS 引入顺序：基础主题 → 组件 → autogen。 */
const GAME_CSS_FILES = [
  "/game-ui/css/92319616.css", // General Themes for SimCity UI
  "/game-ui/css/TextStyles.css",
  "/game-ui/css/TextInputStyles.css",
  "/game-ui/css/ButtonStyles.css",
  "/game-ui/css/WindowStyles.css",
  "/game-ui/css/Animate.css",
  "/game-ui/css/autogenWindowNineSliceStyles.css",
  "/game-ui/css/autogenButtonNineSliceStyles.css",
];

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** 从字符串表挑出适合各槽位的文案：标题取最长、按钮取短句、正文取其余。 */
function pickStrings(entries: Array<[string, string]>) {
  const sorted = [...entries].sort((a, b) => b[1].length - a[1].length);
  const title = sorted[0]?.[1] ?? "";
  const rest = sorted.slice(1).map(([, text]) => text);
  const buttons = rest.filter((text) => text.length <= 18).slice(0, 4);
  const body = rest.filter((text) => text.length > 18).slice(0, 18);
  return { title, buttons, body };
}

export function assembleGameUiHtml(
  spec: ScreenSpec,
  strings: Record<string, string>,
): string {
  const entries = Object.entries(strings).filter(([key]) => key !== "//");
  const { title, buttons, body } = pickStrings(entries);
  const hero = spec.images[0] ?? "";
  const thumbs = spec.images.slice(1);

  const cssLinks = GAME_CSS_FILES.map(
    (href) => `<link rel="stylesheet" href="${href}">`,
  ).join("\n");

  const buttonHtml = buttons
    .map(
      (text) =>
        `<button class="sc-btn" type="button">${escapeHtml(text)}</button>`,
    )
    .join("");

  const bodyHtml = body
    .map(
      (text, index) =>
        `<li class="sc-row${
          index % 2 === 0 ? " alt" : ""
        }"><span class="sc-row-text">${escapeHtml(text)}</span></li>`,
    )
    .join("");

  const thumbHtml = thumbs
    .map(
      (src) =>
        `<figure class="sc-thumb"><img src="${src}" alt="" loading="lazy"></figure>`,
    )
    .join("");

  const emptyNote =
    entries.length === 0
      ? `<p class="sc-empty">此屏幕的 locale 表未命中（FNV-1 映射缺失），以下为占位装配。</p>`
      : "";

  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<base href="/game-ui/css/">
${cssLinks}
<style>
  :root { color-scheme: dark; }
  * { box-sizing: border-box; }
  html, body { margin: 0; height: 100%; }
  body {
    font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
    background:
      radial-gradient(1200px 500px at 50% -10%, #2c4a6e 0%, transparent 60%),
      linear-gradient(180deg, #10161f 0%, #0b1016 100%);
    color: #e8edf3;
    display: flex;
    align-items: stretch;
    justify-content: center;
    padding: 18px;
  }
  .sc-window {
    display: flex;
    flex-direction: column;
    width: 100%;
    max-width: 900px;
    border: 1px solid #3d5a80;
    border-radius: 8px;
    overflow: hidden;
    background: linear-gradient(180deg, rgba(22,32,46,.96), rgba(14,21,31,.96));
    box-shadow: 0 18px 50px rgba(0,0,0,.5);
  }
  .sc-titlebar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    background: linear-gradient(180deg, #3f6ea6, #2b4d78);
    border-bottom: 1px solid #1c2f47;
  }
  .sc-titlebar .dot { width: 9px; height: 9px; border-radius: 50%; background: #9cc4ff; box-shadow: 0 0 8px #6ea8ff; }
  .sc-titlebar h1 { margin: 0; font-size: 15px; font-weight: 600; letter-spacing: .04em; }
  .sc-titlebar .crumb { margin-left: auto; font-size: 10px; opacity: .7; font-family: ui-monospace, monospace; }
  .sc-toolbar { display: flex; gap: 8px; padding: 10px 14px; border-bottom: 1px solid #24354c; flex-wrap: wrap; }
  .sc-btn {
    padding: 5px 14px; font-size: 12px; cursor: default;
    color: #dce8f7; background: linear-gradient(180deg, #456f9e, #33536f);
    border: 1px solid #5b87b5; border-radius: 4px;
  }
  .sc-btn:hover { filter: brightness(1.1); }
  .sc-main { display: grid; grid-template-columns: 1fr 200px; gap: 0; flex: 1; min-height: 0; }
  .sc-list { list-style: none; margin: 0; padding: 6px 0; overflow: hidden; }
  .sc-row { display: flex; align-items: center; padding: 7px 16px; font-size: 12.5px; }
  .sc-row.alt { background: rgba(63,110,166,.10); }
  .sc-row-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sc-side { border-left: 1px solid #24354c; padding: 12px; display: flex; flex-direction: column; gap: 10px; }
  .sc-hero { margin: 0; }
  .sc-hero img { width: 100%; aspect-ratio: 4 / 3; object-fit: cover; border: 1px solid #3d5a80; border-radius: 5px; }
  .sc-thumbs { display: flex; gap: 8px; }
  .sc-thumb { margin: 0; flex: 1; }
  .sc-thumb img { width: 100%; aspect-ratio: 1; object-fit: cover; border: 1px solid #2c415c; border-radius: 4px; }
  .sc-empty { grid-column: 1 / -1; margin: 0; padding: 14px 16px; font-size: 11px; color: #8fa4bd; }
  .sc-statusbar { display: flex; gap: 14px; padding: 7px 14px; border-top: 1px solid #24354c; font-size: 10.5px; color: #8fa4bd; }
</style>
</head>
<body>
  <div class="sc-window" role="presentation">
    <div class="sc-titlebar">
      <span class="dot"></span>
      <h1>${escapeHtml(title || spec.id)}</h1>
      <span class="crumb">${escapeHtml(spec.screen)}</span>
    </div>
    <div class="sc-toolbar">${buttonHtml}</div>
    <div class="sc-main">
      ${emptyNote}
      <ul class="sc-list">${bodyHtml}</ul>
      <aside class="sc-side">
        <figure class="sc-hero"><img src="${hero}" alt=""></figure>
        <div class="sc-thumbs">${thumbHtml}</div>
      </aside>
    </div>
    <div class="sc-statusbar">
      <span>${entries.length} strings</span>
      <span>${spec.group}</span>
      <span>assembled by OpenSCP</span>
    </div>
  </div>
</body>
</html>`;
}

/** 读取屏幕 locale 表（"0x%08X" 键 → 文本）。 */
export async function fetchScreenStrings(
  localePath: string | null,
): Promise<Record<string, string>> {
  if (!localePath) return {};
  try {
    const response = await fetch(localePath);
    if (!response.ok) return {};
    return (await response.json()) as Record<string, string>;
  } catch {
    return {};
  }
}
