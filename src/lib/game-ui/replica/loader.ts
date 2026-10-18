/**
 * ui_replica 复刻数据装载：把 public/game-ui/replica/data/*.js 以 <script> 注入
 * （它们是 `window.HUD_* = {...}` 形式的生成产物，与复刻站完全一致），然后读取全局。
 *
 * 装载时合并 public/game-ui/tools/costs.json（menu_cost_dump 从实际安装包提取的
 * 真实造价索引，按 6C969DEE 名称字串联查 40E1C000/61F0C000 单元配置）——
 * 只回填 cost 为空的条目；维护费与道路造价在静态包中不存在（GlassBox 模拟侧），
 * 保持 null 由 UI 显示「未解析」。
 *
 * 装载结果进程内缓存；测试直接构造 ReplicaData 调 store.hydrate，不走这里。
 */
import { gameAssetDataUrl, gameAssetTgiDataUrl } from "@/lib/game-assets";
import type { ReplicaData, ReplicaSubmenuItem, ReplicaTool } from "./types";

const DATA_BASE = "/game-ui/replica/data/";
const DATA_FILES = [
  "assets.js",
  "locale.js",
  "model.js",
  "layout.js",
  "tools.js",
  "palette.js",
] as const;
const COSTS_URL = "/game-ui/tools/costs.json";
const EXTRA_CONTAINERS_URL = "/game-ui/tools/extra-containers.json";

/** extra-containers.json：运行时装配分类（公共運輸/公園）的静态聚合成员。 */
interface ExtraContainers {
  format?: string;
  tools?: Record<string, ReplicaTool[]>;
  subtabs?: Record<string, { label: string; instances: string[] }[]>;
}

/** 只回填空的分类：这俩面板在 Menu/Menu2 静态数据里 0 成员（按钮序表
 * 0x57572DAC 证实按钮存在，游戏由运行时装配），用聚合产物补齐；
 * 子分类标签同步进 data.subtabs（palette 渲染 tab 行用）。 */
function mergeExtraContainers(
  data: ReplicaData,
  extra: ExtraContainers,
): void {
  for (const [categoryId, list] of Object.entries(extra?.tools ?? {})) {
    if ((data.tools[categoryId]?.length ?? 0) === 0 && list.length > 0) {
      data.tools[categoryId] = list;
    }
  }
  if (extra?.subtabs && Object.keys(extra.subtabs).length > 0) {
    data.subtabs = { ...(data.subtabs ?? {}), ...extra.subtabs };
  }
}

async function loadJson(url: string): Promise<unknown | null> {
  try {
    const response = await fetch(url);
    if (!response.ok) return null;
    return await response.json();
  } catch {
    return null;
  }
}

/** costs.json 的一条命中（unit/config/piece 供溯源）。
 * cost/maintenance = plop 建造价/维护（EcoGame Unit 条目，cToolPlop 语义）；
 * roadCost = 道路 rollover 单价（分段定义 0x0E76A7E2，无键=0 即免费）；
 * configCost = 资产配置 Cost（名称联查兜底）。 */
export interface MenuCostHit {
  cost?: number;
  maintenance?: number;
  roadCost?: number;
  configCost?: number;
  unit?: string;
  config?: string;
  piece?: string;
  name?: string;
}

export interface MenuCostIndex {
  format?: string;
  source?: string;
  costs: Record<string, MenuCostHit>;
  /** 建筑子菜单（menuUnit 子单位反查的模块菜单条目）。 */
  submenus?: Record<string, ReplicaSubmenuItem[]>;
  /** 菜单条目 instance → 建筑子菜单容器 instance（0x0AFE27CA 反查）。 */
  submenuContainers?: Record<string, string>;
}

/** 把造价索引合并进复刻工具表：只填 cost/upkeep 为空的条目。
 * 道路 roadCost（分段定义 0x0E76A7E2，无键=0）也作为显示造价——
 * §0 = 引擎语义「免费」（原版道路铺设不要钱）。纯函数，便于测试。 */
export function mergeMenuCosts(
  tools: Record<string, ReplicaTool[]>,
  index: MenuCostIndex,
): void {
  const costs = index?.costs ?? {};
  for (const list of Object.values(tools)) {
    for (const tool of list) {
      const hit = costs[tool.instance];
      if (!hit) continue;
      if (tool.cost == null) {
        if (hit.cost != null) tool.cost = String(hit.cost);
        else if (hit.roadCost != null) tool.cost = String(hit.roadCost);
        else if (hit.configCost != null) tool.cost = String(hit.configCost);
      }
      if (tool.upkeep == null && hit.maintenance != null) {
        tool.upkeep = String(hit.maintenance);
      }
    }
  }
}

interface BakedSlotNode {
  x: number;
  image: string;
}

/** 布局树里烘焙的工具槽位节点（drawable 首图是 *_default.png 状态图）。 */
function collectBakedSlots(layout: ReplicaData["palette"]["byCategory"][string]["layout"]): BakedSlotNode[] {
  const out: BakedSlotNode[] = [];
  const walk = (node: ReplicaData["layout"]): void => {
    const image = node.drawable?.images?.[0];
    if (image && /_default\.png$/.test(image) && (node.comment ?? "").includes("Tool")) {
      out.push({ x: node.left ?? 0, image });
    }
    for (const child of node.children ?? []) walk(child);
  };
  walk(layout);
  return out;
}

/** 把布局烘焙槽位引用的图片改从玩家安装包解析（game_asset 路径寻址）。
 * 仅当烘焙节点数与条目数一致时按 x 序 ↔ pos 序一一配对；解析失败保留
 * 仓库内置回退路径。 */
async function resolvePaletteIcons(data: ReplicaData): Promise<void> {
  for (const [categoryId, entry] of Object.entries(data.palette?.byCategory ?? {})) {
    const tools = data.tools[categoryId] ?? [];
    if (tools.length === 0) continue;
    const nodes = collectBakedSlots(entry.layout);
    if (nodes.length !== tools.length) continue;
    const sortedNodes = [...nodes].sort((a, b) => a.x - b.x);
    for (const [index, node] of sortedNodes.entries()) {
      const url = await gameAssetDataUrl(node.image);
      if (url) tools[index].preview = url;
    }
  }
}


const ICON_PNG_TYPE = 0x2f7d0004;
const ICON_PNG_GROUP = 0x40e02400;
const MARQUEE_JPG_TYPE = 0x3f8662ea;

/** 全部菜单条目的槽位图标/大图：文件名内嵌资源 instance
 * （tool_icon_<i>.png / tool_<i>.jpg），按 TGI 从玩家安装包解析。 */
async function resolveToolImages(data: ReplicaData): Promise<void> {
  const lists: (ReplicaTool | ReplicaSubmenuItem)[][] = [
    ...Object.values(data.tools),
    ...Object.values(data.submenus ?? {}),
  ];
  for (const tools of lists) {
    for (const tool of tools) {
      const icon = /tool_icon_([0-9A-Fa-f]{8})\.png$/.exec(tool.preview ?? "");
      if (icon) {
        const instance = Number.parseInt(icon[1], 16);
        const url =
          (await gameAssetTgiDataUrl(ICON_PNG_TYPE, instance, ICON_PNG_GROUP)) ??
          (await gameAssetTgiDataUrl(ICON_PNG_TYPE, instance));
        if (url) {
          tool.preview = url;
          tool.previewTgi = { type: ICON_PNG_TYPE, group: ICON_PNG_GROUP, instance };
        }
      } else if (tool.preview && !tool.preview.startsWith("data:")) {
        // Some containers (notably the Region RCI tools) use named layout
        // assets such as RCI_Residential_default.png instead of the
        // tool_icon_<instance>.png convention. Resolve those paths through
        // the same ResourceMan lookup used by palette layout images so the
        // editor and the generated menu both receive the real game bitmap.
        const url = await gameAssetDataUrl(tool.preview);
        if (url) tool.preview = url;
      }
      const marquee = /tool_([0-9A-Fa-f]{8})\.jpg$/.exec(tool.marquee ?? "");
      if (marquee) {
        const instance = Number.parseInt(marquee[1], 16);
        const url = await gameAssetTgiDataUrl(MARQUEE_JPG_TYPE, instance);
        if (url) {
          tool.marquee = url;
          tool.marqueeTgi = { type: MARQUEE_JPG_TYPE, group: 0, instance };
        }
      }
    }
  }
}

/** 布局树的全部图片引用（路径寻址）→ 覆盖 ASSET_MAP 的 basename 条目。
 * HUD 主布局与各调色板布局都走这一条通道；解析失败的引用保留内置回退。 */
async function resolveLayoutAssetMap(data: ReplicaData): Promise<void> {
  const trees: ReplicaData["layout"][] = [data.layout];
  for (const entry of Object.values(data.palette?.byCategory ?? {})) {
    trees.push(entry.layout);
  }
  const refs: string[] = [];
  const walk = (node: ReplicaData["layout"]): void => {
    for (const image of node.drawable?.images ?? []) {
      if (image) refs.push(image);
    }
    for (const child of node.children ?? []) walk(child);
  };
  for (const tree of trees) walk(tree);
  const unique = [...new Set(refs)];
  const chunk = 24;
  for (let i = 0; i < unique.length; i += chunk) {
    await Promise.all(
      unique.slice(i, i + chunk).map(async (ref) => {
        const url = await gameAssetDataUrl(ref);
        if (!url) return;
        const base = ref.split(/[\/]/).pop() ?? ref;
        data.assets[base] = url;
      }),
    );
  }
}

/** 一级分类圆钮图标（model.js 的 iconHash 即 PNG instance）。
 * 渲染层取 icon 路径的文件名查 ASSET_MAP —— 所以解析结果必须写进
 * assets[basename]，不能替换 icon 本身（dataURL 会被 basename 逻辑撕碎）。 */
async function resolveCategoryIcons(data: ReplicaData): Promise<void> {
  for (const category of data.model.categories) {
    const hash = category.iconHash;
    if (!hash) continue;
    const instance = Number.parseInt(hash, 16);
    if (!Number.isFinite(instance)) continue;
    const url = await gameAssetTgiDataUrl(ICON_PNG_TYPE, instance);
    if (url) data.assets[`cat_${hash}.png`] = url;
  }
}

async function loadCostIndex(): Promise<MenuCostIndex> {
  const parsed = (await loadJson(COSTS_URL)) as Partial<MenuCostIndex> | null;
  return { ...parsed, costs: parsed?.costs ?? {} };
}

/** costs.json 的 submenus 挂到 data.submenus（建筑子菜单，Sheet 嵌套编辑用）。 */
function mergeSubmenus(data: ReplicaData, index: MenuCostIndex): void {
  if (index.submenus && Object.keys(index.submenus).length > 0) {
    data.submenus = { ...(data.submenus ?? {}), ...index.submenus };
  }
  if (index.submenuContainers && Object.keys(index.submenuContainers).length > 0) {
    data.submenuContainers = { ...(data.submenuContainers ?? {}), ...index.submenuContainers };
  }
}

let cached: Promise<ReplicaData> | null = null;

export function loadReplicaData(): Promise<ReplicaData> {
  cached ??= (async () => {
    for (const name of DATA_FILES) {
      await injectScript(DATA_BASE + name);
    }
    const data = readGlobals();
    // 增强数据：缺失/损坏都不阻塞菜单装载。
    // 顺序：先补齐运行时装配分类（公共運輸/公園），造价合并才能覆盖到它们。
    mergeExtraContainers(
      data,
      ((await loadJson(EXTRA_CONTAINERS_URL)) as ExtraContainers | null) ?? {},
    );
    const costIndex = await loadCostIndex();
    mergeSubmenus(data, costIndex);
    mergeMenuCosts(data.tools, costIndex);
    // 布局烘焙槽位图标改走玩家安装包解析（Tauri）；浏览器回退仓库内置路径
    await resolvePaletteIcons(data);
    // 全部菜单条目的槽位图标/大图 + 布局图片引用 → 玩家安装包
    await resolveToolImages(data);
    await resolveLayoutAssetMap(data);
    await resolveCategoryIcons(data);
    return data;
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
