/**
 * HUD 布局树 → 嵌套绝对定位 DOM（probe_fire/ui_replica/js/render.js 的 TS 移植，
 * 逻辑逐条对应；子元素坐标系 = 父元素左上角，与游戏引擎一致）。
 *
 * 规则（与游戏 UI 引擎对齐）：
 *   drawable.type=2  images[]      图片，按元素框拉伸（background-size:100% 100%）
 *   drawable.type=5  cssStyles[]   直接挂游戏 CSS 类
 *   长度 >1 视为按钮多态：0 常态 1 悬停 2 按下 3 禁用 4 选中 5 选中悬停 6 选中按下 7 选中禁用
 *   scale / rotation → transform；desiredOpacity → opacity
 */
import { initOffsets, updatePosition } from "@/lib/game-ui/scrui";
import type { ReplicaCategory, ReplicaData, ReplicaModel, ReplicaNode } from "./types";

/** 布局里是 ~token~ 占位，按游戏截图填样例值。 */
const VALUES: Record<string, string> = {
  "~population:number~": "54,666",
  "~money:number~": "422,668", // § 是图标，金额本身不带符号
  "~sim_dateTime:time~": "8:41 PM",
  "~sim_dateTime:shortMonth~": "九月",
  "~year:number~": "3",
  "~amount:number~": "120",
  "~budget:number~": "5,632",
  "~Resource~": "石油",
};

/** 运行时由 JS 填充的容器：保留外壳，不再下探子节点。 */
const SKIP_SUBTREE = new Set<number>([100, 18, 28, 1394, 1908, 301, 973, 1061, 1059, 1057, 8495, 1551]);

/** 模式切换器：三张模式图叠放，游戏只显示当前模式。城市视图默认城市模式。 */
const MODE_ONLY = new Set<number>([1114]); // 1114=city 1019=region 1451=bigbiz

/** 默认隐藏（与 reference/city.png 未展开态比对得出）：
 *  点开面板后才出现的大底衬（743/1065/254）、RCI 外圈内凹底板（839）、
 *  全数据图层钮白色图标（1357）、左侧维度钮告警变体（1346/60/1344/1343/1342）、
 *  教程箭头（769/1551）。 */
const HIDDEN_DEFAULT = new Set<number>([
  743, 1065, 254, 839, 1357, 1346, 60, 1344, 1343, 1342, 769, 1551,
]);

const S_NORMAL = 0;
const S_HOVER = 1;
const S_DOWN = 2;

/** 状态层：游戏里这些子层由运行时按「悬停/选中/告警」切换可见性，
 * 静态布局里没有 opacity 标记，必须按语义先隐掉，否则选中态美术会盖住常态。 */
const HOVER_LAYERS = new Set(["ring", "ring2", "highlight"]);
const SELECT_LAYERS = new Set(["ring on", "frame on", "toggle button2", "icon on (white)"]);
const ALERT_LAYERS = new Set([
  "tab",
  "tool unlocked",
  "critical red frame",
  "caution alert triggered",
  "Need",
  "Opportunity",
  "Category Icon Critical",
  "Category Icon Alert",
  "Category Icon Caution",
]);

/** #1102 城市名由玩家命名；#744 布局写死 "/ hr"，中文版应为 "/ 小時"。 */
const TEXT_OVERRIDES: Record<string, string> = {
  "1102": "榆木林",
  "744": "+~budget:number~ / 小時",
};

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** 布局节点 → 舞台元素（二级面板复用同一套元素构造）。 */
export type NodeElementFactory = (node: ReplicaNode, rect: Rect) => HTMLElement;

export interface HudRender {
  render(stageEl: HTMLElement): {
    w: number;
    h: number;
    /** 分类圆钮行锚点（容器绝对 y + 行内按钮总数），供工作台拊件定位。 */
    categoryRow: { absY: number; count: number } | null;
  };
  /** 布局节点 → 舞台元素（二级面板复用）。 */
  createNodeElement: NodeElementFactory;
  findModule(root: ReplicaNode, path: string): ReplicaNode | null;
  substitute(text: string | null | undefined): string;
  textOf(node: ReplicaNode): string | null;
}

export function createHudRender(data: ReplicaData, options?: {
  /** 额外的一级分类（工作台新建的），追加在圆钮行末位。 */
  extraCategories?: ReplicaCategory[];
}): HudRender {
  const assets = data.assets;
  const locale = data.locale;
  const model: ReplicaModel = {
    categories: [...data.model.categories, ...(options?.extraCategories ?? [])],
    row: data.model.row,
  };

  function substitute(text: string | null | undefined): string {
    return String(text).replace(/~[A-Za-z0-9_:]+~/g, (m) =>
      Object.prototype.hasOwnProperty.call(VALUES, m) ? VALUES[m] : m,
    );
  }

  function textOf(node: ReplicaNode): string | null {
    const override = TEXT_OVERRIDES[String(node.instanceID)];
    if (override != null) return substitute(override);
    if (typeof node.text === "string" && node.text) return substitute(node.text);
    const ls = node.localeString;
    if (ls && ls.tableID && ls.stringID) {
      const table = locale[String(ls.tableID)];
      const raw = table && table[String(ls.stringID).toLowerCase()];
      if (raw) return substitute(raw);
    }
    return null;
  }

  function imageStates(node: ReplicaNode): ({ url: string; name: string } | null)[] | null {
    const dr = node.drawable;
    if (!dr || dr.type !== 2 || !Array.isArray(dr.images) || !dr.images.length) return null;
    const states = dr.images.map((ref): { url: string; name: string } | null => {
      if (typeof ref !== "string" || !ref || ref.toLowerCase() === "none") return null;
      const name = ref.split(/[\\/]/).pop();
      const url = name ? assets[name] : undefined;
      return url && name ? { url, name } : null;
    });
    return states.some(Boolean) ? states : null;
  }

  function cssStates(node: ReplicaNode): string[] | null {
    const dr = node.drawable;
    if (!dr || dr.type !== 5 || !Array.isArray(dr.cssStyles) || !dr.cssStyles.length) return null;
    return dr.cssStyles.slice();
  }

  function layerOf(node: ReplicaNode): "hover" | "select" | "off" | null {
    const c = node.comment;
    if (!c) return null;
    if (HOVER_LAYERS.has(c)) return "hover";
    if (SELECT_LAYERS.has(c)) return "select";
    if (ALERT_LAYERS.has(c) || c === "spec") return "off";
    return null;
  }

  function applyExtras(el: HTMLElement, node: ReplicaNode): void {
    const t: string[] = [];
    if (node.rotation) t.push(`rotate(${Number(node.rotation).toFixed(0)}deg)`);
    if (node.scale && Number(node.scale) !== 1) t.push(`scale(${Number(node.scale).toFixed(4)})`);
    if (t.length) el.style.transform = t.join(" ");
    const layer = layerOf(node);
    if (layer) {
      el.classList.add(`layer-${layer}`);
    } else if (node.desiredOpacity != null) {
      el.style.opacity = String(node.desiredOpacity);
    }
    if (node.zIndex != null) el.style.zIndex = String(node.zIndex);
    const re = node.rootElement;
    if (re) {
      if (re.overflow === "hidden") el.style.overflow = "hidden";
      if (re.backgroundColor) el.style.backgroundColor = re.backgroundColor;
    }
    if (node.overflowType === 1) el.style.overflow = "hidden";
    else if (node.overflowType === 4) el.style.overflow = "auto";
  }

  /** 状态数组里同一个类会重复出现（例如按下的美术沿用常态），所以不能逐槽
   * toggle——后一个槽会把前一个槽刚挂上的同名类删掉。先移除全部去重后的类，
   * 再挂当前态那一个。 */
  function makeStateful(el: HTMLElement, kind: "image" | "css", list: ({ url: string } | string | null)[], selected: boolean): void {
    const n = list.length;
    function at(state: number): number {
      if (selected) {
        if (state === S_HOVER) return Math.min(5, n - 1);
        if (state === S_DOWN) return Math.min(6, n - 1);
        return Math.min(4, n - 1);
      }
      return Math.min(state, n - 1);
    }
    const distinct = kind === "css" ? Array.from(new Set(list.filter(Boolean) as string[])) : null;
    function setAt(i: number): void {
      if (kind === "image") {
        const s = list[i] || list[0];
        el.style.backgroundImage = s ? `url('${(s as { url: string }).url}')` : "none";
      } else {
        for (let k = 0; k < (distinct?.length ?? 0); k++) el.classList.remove(distinct![k]);
        if (list[i]) el.classList.add(list[i] as string);
      }
    }
    let state = S_NORMAL;
    function paint(): void {
      setAt(at(state));
    }
    el.addEventListener("mouseenter", () => {
      state = S_HOVER;
      paint();
    });
    el.addEventListener("mouseleave", () => {
      state = S_NORMAL;
      paint();
    });
    el.addEventListener("mousedown", () => {
      state = S_DOWN;
      paint();
    });
    el.addEventListener("mouseup", () => {
      state = S_HOVER;
      paint();
    });
    paint();
  }

  function createNodeElement(node: ReplicaNode, rect: Rect): HTMLElement {
    const el = document.createElement("div");
    el.className = "hud-el";
    el.style.left = `${rect.x.toFixed(2)}px`;
    el.style.top = `${rect.y.toFixed(2)}px`;
    el.style.width = `${rect.w.toFixed(2)}px`;
    el.style.height = `${rect.h.toFixed(2)}px`;
    applyExtras(el, node);

    const text = textOf(node);
    const imgs = imageStates(node);
    const classes = cssStates(node);
    const isButton = node.buttonType != null || node.buttonGroup != null;

    if (imgs) {
      if (imgs.length > 1 && isButton) {
        el.classList.add("hud-img");
        makeStateful(el, "image", imgs, node.isSelected === true);
      } else {
        el.classList.add("hud-img");
        el.style.backgroundImage = imgs[0] ? `url('${imgs[0].url}')` : "none";
      }
    } else if (classes) {
      if (classes.length === 1 || !isButton) {
        el.classList.add(classes[0]);
      } else {
        makeStateful(el, "css", classes, node.isSelected === true);
      }
    }

    if (text !== null) {
      el.classList.add("hud-text");
      if (node.textStyle) {
        el.classList.add(node.textStyle);
        if (node.textStyle.indexOf("Vmid") >= 0) el.classList.add("hud-vmid");
      }
      el.textContent = text;
      if (node.textColor) el.style.color = node.textColor;
    }
    if (node._module === "Layouts/Palette/CategoryButtonWithIcon2.js") el.classList.add("hud-catbtn");
    // 一级圆钮的白色底盘：hover 时要按游戏动画放大（scale 1 → 1.3 → 1.2，"plump"）
    if (node.comment === "main button") el.classList.add("hud-catmain");
    if (node.comment) el.dataset.comment = node.comment;
    el.dataset.iid = String(node.instanceID);
    return el;
  }

  /** 渲染时记录：分类圆钮容器在舞台上的绝对 y（行拊件定位用）。 */
  let categoryRowAbsY: number | null = null;

  // -------------------------------------------------- 运行时装配：分类圆钮

  /** #10 Category Button Container：游戏在此按分类动态实例化 CategoryButtonWithIcon2。 */
  function instantiateCategoryButtons(
    container: HTMLElement,
    template: ReplicaNode,
    containerAbsX: number,
    containerAbsY: number,
  ): void {
    const row = model.row;
    const cats = model.categories;
    // centerX 是舞台绝对坐标，其余是相对容器的偏移
    const left = row.centerX - containerAbsX - (cats.length * row.pitch) / 2.0;
    const top = row.offsetY || 0;
    void containerAbsY;
    cats.forEach((cat, i) => {
      const clone = JSON.parse(JSON.stringify(template)) as ReplicaNode;
      clone.left = 0;
      clone.top = 0;
      clone.width = 43;
      clone.height = 62;
      const holder = document.createElement("div");
      holder.className = "hud-el cat-btn";
      holder.dataset.category = cat.id;
      holder.title = cat.label;
      // holder 自身就是点击区：给它真实几何（模板放在它内部 (0,0)，舞台位置与之前一致）
      holder.style.left = `${(left + i * row.pitch).toFixed(2)}px`;
      holder.style.top = `${top.toFixed(2)}px`;
      holder.style.width = "43px";
      holder.style.height = "62px";
      container.appendChild(holder);
      placeInto(clone, holder, { x: 0, y: 0, w: 43, h: 62 }, { applyIcon: cat });
    });
  }

  function applyCategoryIcon(node: ReplicaNode, cat: ReplicaCategory): void {
    const walk = (n: ReplicaNode): void => {
      const c = n.comment || "";
      if (c === "icon norm") {
        if (cat.icon) {
          n.drawable = { type: 2, images: [String(cat.icon).split("/").pop() ?? ""] };
        } else {
          n.visibility = false;
        }
      }
      if (c === "icon on (white)" || c.indexOf("Category Icon") === 0) n.visibility = false;
      for (const child of n.children || []) walk(child);
    };
    walk(node);
  }

  // ------------------------------------------------------------ 递归构建

  /** absX/absY：本节点左上角在舞台中的绝对坐标（分类圆钮行要用它换算）。 */
  function placeInto(
    node: ReplicaNode,
    parentEl: HTMLElement,
    rect: Rect,
    opts?: { applyIcon?: ReplicaCategory },
    absX = 0,
    absY = 0,
  ): HTMLElement | null {
    if (node.visibility === false) return null;
    if (
      node.comment === "big Biz mode" ||
      node.comment === "region mode" ||
      node.comment === "city mode"
    ) {
      if (!MODE_ONLY.has(node.instanceID as number)) return null;
    }
    const ax = absX + rect.x;
    const ay = absY + rect.y;
    const el = createNodeElement(node, rect);
    parentEl.appendChild(el);
    if (opts?.applyIcon) applyCategoryIcon(node, opts.applyIcon);

    if (node.comment === "Category Button Container") {
      categoryRowAbsY = ay;
      const mod = findModule(data.layout, "Layouts/Palette/CategoryButtonWithIcon2.js");
      if (mod) {
        instantiateCategoryButtons(el, mod, ax, ay);
      }
      return el;
    }
    if (HIDDEN_DEFAULT.has(node.instanceID as number)) el.classList.add("hud-hidden");
    if (SKIP_SUBTREE.has(node.instanceID as number) || node._missing) return el;

    const kids = node.children ?? [];
    for (const child of kids) {
      const r = updatePosition(child, rect.w, rect.h);
      placeInto(child, el, r, undefined, ax, ay);
    }
    return el;
  }

  function findModule(root: ReplicaNode, path: string): ReplicaNode | null {
    let found: ReplicaNode | null = null;
    const walk = (n: ReplicaNode): void => {
      if (n._module === path) found = n;
      for (const c of n.children || []) walk(c);
    };
    walk(root);
    return found;
  }

  function render(stageEl: HTMLElement): {
    w: number;
    h: number;
    categoryRow: { absY: number; count: number } | null;
  } {
    const root = data.layout;
    const w = stageEl.clientWidth;
    const h = stageEl.clientHeight;
    designPass(root, w, h);
    const r = updatePosition(root, w, h);
    placeInto(root, stageEl, r);
    return {
      w,
      h,
      categoryRow:
        categoryRowAbsY == null
          ? null
          : { absY: categoryRowAbsY, count: model.categories.length },
    };
  }

  return { render, createNodeElement, findModule, substitute, textOf };
}

/** 设计期：按各节点 authored 尺寸自上而下算四向偏移（scrui 两段式的第一段）。
 * 二级面板是另一棵布局树，打开前同样要跑一遍。 */
export function designPass(root: ReplicaNode, stageW: number, stageH: number): void {
  const walk = (node: ReplicaNode, pw: number, ph: number): void => {
    initOffsets(node, pw, ph);
    for (const child of node.children ?? []) walk(child, node.width ?? 0, node.height ?? 0);
  };
  walk(root, stageW, stageH);
}
