/**
 * 生成 public/game-ui/tools/extra-containers.json：
 * 公共運輸(CBC27856) / 公園(899BEA70) 两个一级分类在 Menu/Menu2 静态数据里
 * 没有直接成员（0x57572DAC 的 0x09A4A024 按钮序表证实按钮存在，调色板由运行时
 * 装配）。本脚本按可验证的容器关系聚合成员，合并 menu_asset_extract 产出的
 * extra-meta.json（名称/描述/统计行/槽位图标映射），产出 ReplicaTool 形状的
 * 条目，loader 在装载时回填这两个空分类。
 *
 * 聚合规则（2026-10-18 对拍 tools.json + 游戏截图定案）：
 * - 公共運輸带子分类标签（游戏内：公車/路面電車/火車/船/飛機/磁浮列車）；
 *   静态可聚合的为前四个（飛機/磁浮列車无静态成员）。
 * - 公園 = parent ∈ 四个公园子菜单根；游戏内的价格档标签（基本($)/運動($$)…）
 *   是纯运行时 UI，静态无对应容器，本轮保持单列。
 *
 * 用法：node tools/generate-extra-containers.cjs
 */
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const tools = JSON.parse(fs.readFileSync(path.join(root, "public/game-ui/tools/tools.json"), "utf8"));
const locale = (table) =>
  JSON.parse(fs.readFileSync(path.join(root, `public/game-ui/locale/${table}.json`), "utf8"));
const names = locale("6C969DEE");
const descs = locale("50AA0BEA");
const unlocks = locale("4B54417A");
const name = (id) => names["0x" + (id || "").toLowerCase()] ?? "";

const ASSETS = path.join(root, "public/game-ui/replica/assets");
const assetExists = (file) => fs.existsSync(path.join(ASSETS, file));
const meta = JSON.parse(fs.readFileSync(path.join(root, "public/game-ui/tools/extra-meta.json"), "utf8"));

/** 公共運輸的子分类（标签对拍游戏截图；飞艇/磁浮无静态成员不设）。 */
const TRANSIT_SUBTABS = [
  { label: "公車", via: ["2A99C945"] },
  { label: "路面電車", via: ["A63DDC77"] },
  { label: "火車", via: ["4B277752"] },
  { label: "船", via: ["DC8602C9", "6AFE622F"] },
  // 飛機（飛艇庫，Airships DLC）與磁浮列車（EP1：軌道/車站；車庫單列容器）
  // 2026-10-20 補齊——此前「无静态成员不设」的判断有误，容器实为聚合可达。
  { label: "飛機", via: ["3E9C273B"] },
  { label: "磁浮列車", via: ["FD631707", "5817C009", "F79888D2"] },
];
const TRANSIT_CONTAINERS = TRANSIT_SUBTABS.flatMap((t) => t.via);
const PARK_ROOTS = ["C725C506", "F1035727", "4EEB801B", "F970A822"];

function decorate(row) {
  const instance = row.instance;
  const extra = meta.meta?.[instance] ?? {};
  const iconKey = meta.iconMap?.[instance];
  const preview =
    (iconKey && assetExists(`tool_icon_${iconKey}.png`)
      ? `/game-ui/replica/assets/tool_icon_${iconKey}.png`
      : null) ?? null;
  const marquee = assetExists(`tool_${instance}.jpg`)
    ? `/game-ui/replica/assets/tool_${instance}.jpg`
    : null;
  // 统计行：值 + 后缀按游戏弹窗形态合并（公車容量 = 40 個模擬市民）；
  // 值缺失（运行时量）→ null，与复刻数据语义一致。
  const stats = (extra.stats ?? []).map((s) => ({
    label: s.label,
    value: s.value == null ? null : `${s.value}${s.suffix ?? ""}`,
  }));
  return {
    instance,
    label: extra.name || name(row.titleTextId) || "",
    desc: extra.desc ?? descs["0x" + (row.descTextId || "").toLowerCase()] ?? "",
    unlock: unlocks["0x" + (row.descTextId || "").toLowerCase()] ?? "",
    stats: stats.length > 0 ? stats : [],
    cost: null,
    upkeep: null,
    preview,
    marquee,
    locked: row.hardGate === "1",
  };
}

function membersByCategories(containers) {
  const seen = new Map();
  for (const row of tools.tools) {
    const cats = String(row.uiCategories ?? "").split(",");
    if (!containers.some((c) => cats.includes(c))) continue;
    if (!seen.has(row.instance)) seen.set(row.instance, row);
  }
  return [...seen.values()];
}

function membersByParents(roots) {
  return tools.tools.filter((row) => roots.includes(row.parent));
}

const transitRows = membersByCategories(TRANSIT_CONTAINERS);
const transitTools = transitRows.map(decorate);
// 子分类 → 成员 instance 列表（palette 按 tab 过滤）
const transitSubtabs = TRANSIT_SUBTABS.map((tab) => ({
  label: tab.label,
  instances: transitRows
    .filter((row) => String(row.uiCategories ?? "").split(",").some((c) => tab.via.includes(c)))
    .map((row) => row.instance),
})).filter((tab) => tab.instances.length > 0);

const out = {
  format: "openscp-extra-containers/2",
  source:
    "tools.json aggregated + extra-meta.json (menu_asset_extract): CBC27856 = uiCategories in transit module containers (subtabs per game); 899BEA70 = parent in park submenu roots.",
  containers: {
    CBC27856: { via: TRANSIT_CONTAINERS, by: "uiCategories" },
    "899BEA70": { via: PARK_ROOTS, by: "parent" },
  },
  subtabs: {
    CBC27856: transitSubtabs,
  },
  tools: {
    CBC27856: transitTools,
    "899BEA70": membersByParents(PARK_ROOTS).map(decorate),
  },
};

const outPath = path.join(root, "public/game-ui/tools/extra-containers.json");
fs.writeFileSync(outPath, JSON.stringify(out, null, 2) + "\n");
console.log(
  `CBC27856 公共運輸: ${transitTools.length} 条（${transitSubtabs.length} 个子分类）；899BEA70 公園: ${out.tools["899BEA70"].length} 条 → ${outPath}`,
);
const withIcon = [...transitTools, ...out.tools["899BEA70"]].filter((t) => t.preview).length;
const withName = [...transitTools, ...out.tools["899BEA70"]].filter((t) => t.label).length;
const withStats = [...transitTools, ...out.tools["899BEA70"]].filter((t) => t.stats.length > 0).length;
console.log(`图标 ${withIcon}、名称 ${withName}、统计行 ${withStats} / 共 ${transitTools.length + out.tools["899BEA70"].length}`);
