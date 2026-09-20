/**
 * ui_replica 复刻数据装载：把 public/game-ui/replica/data/*.js 以 <script> 注入
 * （它们是 `window.HUD_* = {...}` 形式的生成产物，与复刻站完全一致），然后读取全局。
 *
 * 装载结果进程内缓存；测试直接构造 ReplicaData 调 store.hydrate，不走这里。
 */
import type { ReplicaData } from "./types";

const DATA_BASE = "/game-ui/replica/data/";
const DATA_FILES = [
  "assets.js",
  "locale.js",
  "model.js",
  "layout.js",
  "tools.js",
  "palette.js",
] as const;

let cached: Promise<ReplicaData> | null = null;

export function loadReplicaData(): Promise<ReplicaData> {
  cached ??= (async () => {
    for (const name of DATA_FILES) {
      await injectScript(DATA_BASE + name);
    }
    return readGlobals();
  })();
  // 失败时清空缓存：否则被缓存的是 rejected promise，一次瞬态失败
  // （dev server 抖动等）之后本次会话永远装载失败。
  return cached.catch((cause) => {
    cached = null;
    throw cause;
  });
}

function injectScript(src: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = src;
    script.addEventListener("load", () => resolve());
    script.addEventListener("error", () => reject(new Error(`复刻数据脚本加载失败：${src}`)));
    document.head.appendChild(script);
  });
}

interface ReplicaGlobals {
  HUD_LAYOUT?: ReplicaData["layout"];
  ASSET_MAP?: ReplicaData["assets"];
  HUD_LOCALE?: ReplicaData["locale"];
  CATEGORY_MODEL?: ReplicaData["model"];
  HUD_TOOLS?: ReplicaData["tools"];
  HUD_PALETTE?: ReplicaData["palette"];
}

function readGlobals(): ReplicaData {
  const global = window as unknown as ReplicaGlobals;
  if (!global.HUD_LAYOUT || !global.CATEGORY_MODEL || !global.HUD_PALETTE || !global.HUD_TOOLS) {
    throw new Error("复刻数据不完整：缺少 HUD_LAYOUT / CATEGORY_MODEL / HUD_PALETTE / HUD_TOOLS");
  }
  return {
    layout: global.HUD_LAYOUT,
    assets: global.ASSET_MAP ?? {},
    locale: global.HUD_LOCALE ?? {},
    model: global.CATEGORY_MODEL,
    tools: global.HUD_TOOLS,
    palette: global.HUD_PALETTE,
  };
}
