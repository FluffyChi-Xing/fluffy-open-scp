/**
 * scrui 布局引擎（逆向自游戏 App 包 `7D14BE70.js`——窗口系统本体是 JavaScript，
 * 不是机器码；见 docs/roadmap §49.8）。
 *
 * 两段式模型（与引擎一致）：
 * 1. 设计期 `initOffsets`：按父容器的**设计尺寸**（1024×793）把 authored
 *    几何换算成四向偏移，只算一次；
 * 2. 运行期 `updatePosition`：窗口 resize 后按父容器**实际尺寸**用偏移重算
 *    位置/大小，自上而下级联。
 *
 * pin 枚举（scrui.cControlBase）：
 *   0 = Left（绝对左上） 1 = Right（锚另一边） 2 = Stretch（一边固定一边随父）
 *   3 = Center 4 = Fill（铺满父） 5 = Proportional（两端按比例）
 */

export interface LayoutNode {
  instanceID?: number;
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
  drawable?: { type?: number; images?: string[] } | null;
  children?: LayoutNode[];
}

export const PIN_LEFT = 0;
export const PIN_RIGHT = 1;
export const PIN_STRETCH = 2;
export const PIN_CENTER = 3;
export const PIN_FILL = 4;
export const PIN_PROPORTIONAL = 5;

/** 引擎设计画布（布局树 authored 坐标系）。 */
export const DESIGN_WIDTH = 1024;
export const DESIGN_HEIGHT = 793;

interface Offsets {
  lo: number;
  ro: number;
  to: number;
  bo: number;
}

const OFFSETS = new WeakMap<LayoutNode, Offsets>();

export function initOffsets(
  node: LayoutNode,
  parentWidth: number,
  parentHeight: number,
): void {
  const l = node.left ?? 0;
  const t = node.top ?? 0;
  const r = l + (node.width ?? 0);
  const b = t + (node.height ?? 0);
  const hp = node.horizontalPinType ?? PIN_LEFT;
  const vp = node.verticalPinType ?? PIN_LEFT;
  const off: Offsets = { lo: 0, ro: 0, to: 0, bo: 0 };
  if (hp === PIN_RIGHT || hp === PIN_STRETCH) {
    off.lo = l;
    off.ro = r - parentWidth;
  } else if (hp === PIN_CENTER) {
    off.lo = l - parentWidth / 2;
    off.ro = r - parentWidth / 2;
  } else if (hp === PIN_PROPORTIONAL) {
    off.lo = l - (node.leftProportionalRatio ?? 0) * parentWidth;
    off.ro = r - (node.rightProportionalRatio ?? 0) * parentWidth;
  }
  if (vp === PIN_RIGHT || vp === PIN_STRETCH) {
    off.to = t;
    off.bo = b - parentHeight;
  } else if (vp === PIN_CENTER) {
    off.to = t - parentHeight / 2;
    off.bo = b - parentHeight / 2;
  } else if (vp === PIN_PROPORTIONAL) {
    off.to = t - (node.topProportionalRatio ?? 0) * parentHeight;
    off.bo = b - (node.bottomProportionalRatio ?? 0) * parentHeight;
  }
  OFFSETS.set(node, off);
}

export function updatePosition(
  node: LayoutNode,
  parentWidth: number,
  parentHeight: number,
): { x: number; y: number; w: number; h: number } {
  const off =
    OFFSETS.get(node) ??
    ({ lo: 0, ro: 0, to: 0, bo: 0 } as Offsets);
  const l = node.left ?? 0;
  const t = node.top ?? 0;
  let x = l;
  let y = t;
  let w = node.width ?? 0;
  let h = node.height ?? 0;
  const hp = node.horizontalPinType ?? PIN_LEFT;
  const vp = node.verticalPinType ?? PIN_LEFT;
  switch (hp) {
    case PIN_STRETCH:
      w = Math.max(parentWidth - off.lo + off.ro, 0);
      break;
    case PIN_RIGHT:
      x = parentWidth - w + off.ro;
      break;
    case PIN_CENTER:
      x = parentWidth / 2 + off.lo;
      w = parentWidth / 2 + off.ro - x;
      break;
    case PIN_FILL:
      x = 0;
      w = parentWidth;
      break;
    case PIN_PROPORTIONAL: {
      x = (node.leftProportionalRatio ?? 0) * parentWidth + off.lo;
      w = off.ro + (node.rightProportionalRatio ?? 0) * parentWidth - x;
      break;
    }
    default:
      break;
  }
  switch (vp) {
    case PIN_STRETCH:
      h = Math.max(parentHeight - off.to + off.bo, 0);
      break;
    case PIN_RIGHT:
      y = parentHeight - h + off.bo;
      break;
    case PIN_CENTER:
      y = parentHeight / 2 + off.to;
      h = parentHeight / 2 + off.bo - y;
      break;
    case PIN_FILL:
      y = 0;
      h = parentHeight;
      break;
    case PIN_PROPORTIONAL: {
      y = (node.topProportionalRatio ?? 0) * parentHeight + off.to;
      h = off.bo + (node.bottomProportionalRatio ?? 0) * parentHeight - y;
      break;
    }
    default:
      break;
  }
  return { x, y, w, h };
}

export interface PlacedRect {
  id: string;
  comment: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface PlacedImage {
  id: string;
  comment: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** 解析后的提取资源路径。 */
  src: string;
}

/**
 * 两段式解析：先设计尺寸自上而下 Init 偏移，再以实际根尺寸 Update 级联。
 * 返回 instanceID → 绝对矩形 + 可渲染图片清单（树序 = 绘制序）。
 */
export function resolveLayout(
  root: LayoutNode,
  stageWidth: number,
  stageHeight: number,
): { rects: Map<string, PlacedRect>; images: PlacedImage[] } {
  const rects = new Map<string, PlacedRect>();
  const images: PlacedImage[] = [];

  const design = (node: LayoutNode, pw: number, ph: number): void => {
    initOffsets(node, pw, ph);
    for (const child of node.children ?? []) {
      design(child, node.width ?? 0, node.height ?? 0);
    }
  };
  design(root, stageWidth, stageHeight);

  const runtime = (
    node: LayoutNode,
    px: number,
    py: number,
    pw: number,
    ph: number,
  ): void => {
    const { x, y, w, h } = updatePosition(node, pw, ph);
    const ax = px + x;
    const ay = py + y;
    const id = String(node.instanceID ?? "");
    if (id) {
      rects.set(id, {
        id,
        comment: node.comment ?? "",
        x: ax,
        y: ay,
        w,
        h,
      });
    }
    if (node.visibility === false) {
      return; // 隐藏子树整体跳过（与引擎 visibility 语义一致）
    }
    const nodeImages = node.drawable?.images;
    if (nodeImages && nodeImages.length > 0) {
      const resolved = nodeImages
        .filter((ref): ref is string => typeof ref === "string")
        .map((ref) => assetPathForRef(ref))
        .filter((src): src is string => src !== null);
      if (resolved.length > 0) {
        images.push({
          id,
          comment: node.comment ?? "",
          x: ax,
          y: ay,
          w,
          h,
          src: resolved[0],
        });
      }
    }
    for (const child of node.children ?? []) {
      runtime(child, ax, ay, w, h);
    }
  };
  runtime(root, 0, 0, stageWidth, stageHeight);

  return { rects, images };
}

/** FNV-1（小写）：与游戏 UI 资源命名一致（instance = FNV-1(去扩展名)）。 */
export function assetPathForRef(ref: string | null | undefined): string | null {
  if (!ref) return null;
  const base = ref.split(/[\\/]/).pop() ?? "";
  const dot = base.lastIndexOf(".");
  if (dot <= 0) return null;
  const stem = base.slice(0, dot);
  const ext = base.slice(dot + 1).toLowerCase();
  if (ext !== "png" && ext !== "jpg" && ext !== "gif") return null;
  let hash = 0x811c9dc5;
  for (const ch of stem) {
    hash = Math.imul(hash, 0x01000193);
    hash ^= ch.charCodeAt(0) & 0xff;
  }
  hash = hash >>> 0;
  return `/game-ui/${ext}/00000000_${hash.toString(16).toUpperCase().padStart(8, "0")}.${ext}`;
}
