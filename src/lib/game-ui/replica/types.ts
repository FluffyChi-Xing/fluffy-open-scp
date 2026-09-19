/**
 * ui_replica 复刻数据的类型描述。
 *
 * 数据文件（public/game-ui/replica/data/*.js）由 probe_fire/ui_replica/tools/build.py
 * 从逆向产物生成；本目录只消费，不再重复生成。结构与 probe_fire/ui_replica 中的
 * render.js / palette.js 所读取的字段一一对应。
 */

/** 布局树节点（globalui2 裁剪 + 子模块内联后的产物）。 */
export interface ReplicaNode {
  instanceID?: number | string;
  comment?: string;
  left?: number;
  top?: number;
  width?: number;
  height?: number;
  horizontalPinType?: number;
  verticalPinType?: number;
  leftProportionalRatio?: number;
  rightProportionalRatio?: number;
  topProportionalRatio?: number;
  bottomProportionalRatio?: number;
  visibility?: boolean;
  drawable?: ReplicaDrawable | null;
  /** 布局自带动画（面板 Shown/Hidden 触发器）；渲染只关心动画目标节点清单。 */
  animations?: ReplicaAnimation[];
  text?: string;
  textStyle?: string;
  textColor?: string;
  localeString?: { tableID?: number | string; stringID?: number | string } | null;
  buttonType?: number | null;
  buttonGroup?: number | null;
  isSelected?: boolean;
  rotation?: number;
  scale?: number;
  desiredOpacity?: number;
  zIndex?: number;
  rootElement?: { overflow?: string; backgroundColor?: string } | null;
  overflowType?: number;
  layoutPath?: string;
  /** build.py 内联子模块时打的模块路径标记。 */
  _module?: string;
  /** layoutPath 指向的模块缺失。 */
  _missing?: boolean;
  children?: ReplicaNode[];
}

export interface ReplicaDrawable {
  type?: number;
  images?: string[];
  cssStyles?: string[];
}

export interface ReplicaAnimation {
  controlTimelines?: { animatedControlIID?: number }[];
}

/** 一级分类（Menu/Menu2 推导，id = 分类 property instance）。 */
export interface ReplicaCategory {
  id: string;
  label: string;
  /** assets 下的图标路径（已改写为应用内绝对路径）；null = 无静态图标。 */
  icon: string | null;
  iconHash: string | null;
}

/** 一级圆钮行几何（1600×900 舞台标定值）。 */
export interface ReplicaRow {
  pitch: number;
  size: number;
  centerX: number;
  offsetY: number;
}

/** 二级面板槽位行几何（相对面板菜单条的标定值）。 */
export interface ReplicaSlotRow {
  pitch: number;
  startX: number;
  slotW: number;
  slotH: number;
  topInBar: number;
}

/** 二级槽位的一个工具条目。 */
export interface ReplicaTool {
  /** 工具 property 的 instance（0x%08X 大写十六进制）。 */
  instance: string;
  /** locale 解析后的显示名；可能为空串。 */
  label: string;
  /** locale 解析后的功能描述（0x0A09F5FB → 表 0x50AA0BEA）；可能为空串。 */
  desc?: string;
  /** 解锁条件文案（0x0DE84DDC → 表 0x4B54417A）；仅锁定项常有值。 */
  unlock?: string;
  /** 槽位预览图；缺失时渲染层回退到 tool_placeholder.png。 */
  preview: string | null;
  marquee: string | null;
  locked: boolean;
}

export interface ReplicaModel {
  categories: ReplicaCategory[];
  row: ReplicaRow;
}

export interface ReplicaPaletteEntry {
  layoutInstance: string;
  layout: ReplicaNode;
  slotRow: ReplicaSlotRow;
}

export interface ReplicaPalette {
  byCategory: Record<string, ReplicaPaletteEntry>;
  row: ReplicaSlotRow;
}

/** data/*.js 六个全局的聚合。 */
export interface ReplicaData {
  layout: ReplicaNode;
  assets: Record<string, string>;
  locale: Record<string, Record<string, string>>;
  model: ReplicaModel;
  tools: Record<string, ReplicaTool[]>;
  palette: ReplicaPalette;
}
