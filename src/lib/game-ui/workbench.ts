/**
 * UI 工作台数据层：游戏菜单的真实数据源 + 装配几何。
 *
 * 数据链（全部离线取证得出，见 docs/roadmap §49）：
 * - 菜单/工具定义 = Menu/Menu2 property（0x00B1B104, InstanceType 0x8A01/0xC900），
 *   标题 Text → locale 表 0x6C969DEE（繁中），槽位图标 = kPropToolIconKey
 *   （0x0977AA8F，组 0x40E02400 的等轴模型渲染 PNG；0x09756950–55 的六态图标键
 *   按 TGI 在全部包中不落地，2026-09 复盘定案）。
 * - HUD 布局树 = `Layouts/GlobalUI2.js`（实为 JSON；type 0x67771F5C，
 *   group 0x0B074E5A），设计分辨率 1024×793。
 * - 图片资源按 **instance = FNV-1(小写去扩展名)** 命名存储，与提取目录
 *   `public/game-ui/<ext>/<group>_<instance>.<ext>` 一一对应。
 *
 * preview 为 null 的条目（如公园回填的旧数据）：槽位/缩略图回退 tool_placeholder.png。
 */

export interface WorkbenchTool {
  /** 工具 property 的 instance（0x%08X 大写十六进制）。 */
  id: string;
  /** locale 解析后的显示名（繁中）；解析失败时为 instance 占位。 */
  label: string;
  /** uiToolPosition（0x0DC1E3E0），菜单内排序键。 */
  pos: number;
  /** 图标覆盖（FIcon 名）；null = 未覆盖，展示层回退到真实槽位图 preview。 */
  icon: string | null;
  /** 真实槽位图（kPropToolIconKey 提取物，/game-ui/replica/assets/…）；null = 无。 */
  preview?: string | null;
  /** rollover 大图（kPropToolMarqueeImage，454×263）；提示框媒体图。 */
  marquee?: string | null;
  /** locale 解析的功能描述（0x0A09F5FB）；提示框正文。 */
  desc?: string;
  /** 解锁条件文案（0x0DE84DDC）；仅锁定项常有值。 */
  unlock?: string;
  /** hardGate（0x0975695F）标记的锁定项。 */
  locked?: boolean;
  /** 名称来源：locale = 游戏字符串表；unresolved = 尚无译名。 */
  source: "locale" | "unresolved";
}

export interface WorkbenchCategory {
  id: string;
  label: string;
  icon: string | null;
  items: WorkbenchTool[];
}

export interface WorkbenchData {
  meta: {
    source: string;
    toolCount: number;
    designResolution: [number, number];
    note: string;
  };
  assets: Record<string, string | null>;
  university: { label: string; tools: WorkbenchTool[] };
  city: { categories: WorkbenchCategory[] };
}

/** 工作台舞台 = 参考截图分辨率（1600×900），所有几何按此坐标标注。 */
export const STAGE_WIDTH = 1600;
export const STAGE_HEIGHT = 900;

/** 城市分类 → FIcon 名（图标库没有的语义用最接近的替代，见 §49）。 */
export const CATEGORY_ICONS: Record<string, string> = {
  road: "GitBranch",
  power: "Zap",
  water: "Cloud",
  sewage: "RefreshCw",
  garbage: "Trash2",
  fire: "Bell",
  health: "Heart",
  safety: "Shield",
  park: "Sparkles",
  education: "BookOpen",
  trade: "Boxes",
  landmark: "MapPin",
  mayor: "House",
};

/** 条目展示图标：编辑里填了 FIcon 名用之（资源路径视为「非 FIcon」），
 * 否则占位。 */
export const PLACEHOLDER_ICON = "Box";

/** thumb 走 FIcon 时的名字：仅认 FIcon 覆盖名，路径/空值都归占位。 */
export function toolIconName(tool: { icon: string | null }): string {
  const icon = tool.icon?.trim();
  if (icon && !icon.startsWith("/")) return icon;
  return PLACEHOLDER_ICON;
}

/** 条目真实槽位图：有 FIcon 覆盖时不参与展示，否则回真实提取物路径。 */
export function toolPreview(tool: { icon: string | null; preview?: string | null }): string | null {
  if (tool.icon) return null;
  return tool.preview ?? null;
}

/** FNV-1（乘后异或），与游戏资源命名一致；入参先转小写。
 *  乘法必须走 Math.imul——普通乘法在 2^53 以上丢低位，会算出错误哈希。 */
export function fnv1Lower(name: string): number {
  let hash = 0x811c9dc5;
  for (const char of name.toLowerCase()) {
    hash = Math.imul(hash, 0x01000193);
    hash ^= char.charCodeAt(0) & 0xff;
  }
  return hash >>> 0;
}

const EXT_BY_SIGNATURE: Record<string, string> = {
  png: "png",
  jpg: "jpg",
  gif: "gif",
};

/**
 * 布局树里的图片引用（`Graphics/HUD/puck-base.png`）→ 提取物路径。
 * 游戏 UI 的 74 处引用在提取物中 100% 命中。
 */
export function assetPathForRef(ref: string): string | null {
  const base = ref.split(/[\\/]/).pop() ?? "";
  const dot = base.lastIndexOf(".");
  if (dot <= 0) return null;
  const stem = base.slice(0, dot);
  const ext = EXT_BY_SIGNATURE[base.slice(dot + 1).toLowerCase()];
  if (!ext) return null;
  return `/game-ui/${ext}/00000000_${fnv1Lower(stem).toString(16).toUpperCase().padStart(8, "0")}.${ext}`;
}

/** 单条菜单项的编辑覆盖（落库前暂存在工作台内）。 */
export interface ToolEdit {
  label?: string;
  icon?: string | null;
  pos?: number;
}

/** 应用编辑后的展示视图。 */
export function applyEdit(
  tool: WorkbenchTool,
  edit: ToolEdit | undefined,
): WorkbenchTool {
  if (!edit) return tool;
  return {
    ...tool,
    label: edit.label ?? tool.label,
    icon: edit.icon !== undefined ? edit.icon : tool.icon,
    pos: edit.pos ?? tool.pos,
  };
}

/** 按 pos 稳定排序。 */
export function sortTools(tools: WorkbenchTool[]): WorkbenchTool[] {
  return [...tools].sort((a, b) => a.pos - b.pos || a.id.localeCompare(b.id));
}
