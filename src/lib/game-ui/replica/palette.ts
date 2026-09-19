/**
 * 二级面板（工具调色板）—— probe_fire/ui_replica/js/palette.js 的 TS 移植。
 *
 * 开/关机制来自布局数据本身：每个一级分类对象带 kPropToolCategoryPaletteLayout
 * 指向一个布局资源，该布局根节点上挂着动画，触发器为 Shown(12) / Hidden(13) ——
 * 「显示该布局」= 打开，「隐藏」= 关闭且同一条时间轴反向播放。
 *
 * 动画实测参数（panel reveal up，367ms）：
 *   面板本体 controlTop 80 → -38（升起 118px）、controlOpacity 0 → 1
 *
 * 相对 ui_replica 的差异（仅工作台钩子，不改变默认观感）：
 *   - 槽位行工具清单可由调用方覆盖（工作台的编辑覆盖/新增条目）；
 *   - 槽位可挂点击回调与「新增条目」删除角标。
 */
import { designPass, type NodeElementFactory } from "./render";
import { updatePosition } from "@/lib/game-ui/scrui";
import type { ReplicaData, ReplicaNode, ReplicaSlotRow, ReplicaTool } from "./types";

export interface SlotTool extends ReplicaTool {
  /** 本条是工作台新增的（未落库）→ 显示删除角标。 */
  isNew?: boolean;
}

export interface PaletteHooks {
  /** 槽位被点击（默认无）。 */
  onSlotClick?: (categoryId: string, tool: SlotTool) => void;
  /** 工作台删除新增条目（仅 isNew 槽位的角标触发）。 */
  onRemoveItem?: (categoryId: string, tool: SlotTool) => void;
  /** 槽位行工具清单覆盖（工作台编辑覆盖/新增条目后的最终视图）。 */
  getTools?: (categoryId: string) => SlotTool[] | undefined;
  /** 面板关闭（面板自带关闭钮触发；调用方负责一级圆钮取消选中）。 */
  onClose?: () => void;
}

export interface HudPalette {
  init(stage: HTMLElement): void;
  open(categoryId: string, label: string): boolean;
  close(): void;
  isOpen(categoryId: string): boolean;
  current(): string | null;
  /** 用当前数据重绘打开中的面板（工作台编辑覆盖实时生效；不重播开合动画）。 */
  repaint(): void;
}

/** 面板根节点是 1024×150 且 pin FILL/STRETCH：它的子节点在设计期以 150 为父高算偏移，
 * 所以宿主盒子高度必须给 150，内部布局才与设计一致；宽度铺满舞台。
 *
 * 垂直位置不能写死：各布局里 #2 的 top 不同（-38 / +80 / -56）。改为按「面板条落点」
 * 锚定 —— 找到背景条（CSS Palette_tab）算出的屏幕 y，再把整个 body 平移，
 * 使它对到参考图实测的 745。 */
const PANEL = { x: 0, h: 150, barY: 745 };

export function createHudPalette(
  data: ReplicaData,
  createNodeElement: NodeElementFactory,
  hooks: PaletteHooks = {},
): HudPalette {
  let host: HTMLElement | null = null; // 舞台上的固定容器（负责定位与显隐）
  let body: HTMLElement | null = null; // 当前面板的布局树
  let currentId: string | null = null;
  let currentLabel = "";
  /** 面板动画（panel reveal up）驱动的不透明度/位移由 show/hide 过渡负责，
   * 这些节点在布局里写的 desiredOpacity（初始 0）只是动画起点，不能当静态值画死，
   * 否则整块面板会保持透明 —— 道路/灾难两种布局的 #2 就带 desiredOpacity: 0。 */
  let animTargets = new Set<string>();

  function init(stage: HTMLElement): void {
    host = document.createElement("div");
    host.id = "palette";
    host.className = "hud-palette";
    // 插到舞台最底层：面板在真机里位于工具条**后面**，一级圆钮要压在它上面，
    // 否则面板的底衬会盖住圆钮行。
    stage.insertBefore(host, stage.firstChild);
  }

  function clear(): void {
    body?.remove();
    body = null;
    currentId = null;
  }

  /** 把一个分类的面板布局渲染进 host。 */
  function paint(categoryId: string, label: string): boolean {
    const entry = data.palette.byCategory[categoryId];
    if (!entry || !host) return false;
    const stageW = host.parentElement?.clientWidth ?? 1600;
    const tree = entry.layout;
    designPass(tree, stageW, PANEL.h);

    const barY = measureBarY(tree, stageW, PANEL.h);
    body = document.createElement("div");
    body.className = "hud-palette-body";
    body.style.left = `${PANEL.x}px`;
    body.style.top = `${PANEL.barY - barY}px`;
    body.style.width = `${stageW}px`;
    body.style.height = `${PANEL.h}px`;
    host.appendChild(body);

    const r = updatePosition(tree, stageW, PANEL.h);
    animTargets = animatedIids(tree);
    place(tree, body, r, 0, 0);

    attachLabel(body, entry.layout, label);
    attachCloseButton(body);
    attachSlotRow(body, categoryId, entry.slotRow, barY);
    return true;
  }

  /** 面板自带的关闭钮（Layouts/GlobalUI/CloseButton.js）。
   * 点它和再点一次一级圆钮是同一件事：隐藏面板（动画反向播放）。
   *
   * 注意：不能按节点号找 —— 关闭钮在各布局里的 instanceID 不同，
   * 按模块标识（place 里打的 .hud-close）才是布局无关的。 */
  function attachCloseButton(parentEl: HTMLElement): void {
    parentEl.querySelectorAll(".hud-close").forEach((el) => {
      el.addEventListener("click", (e) => {
        e.stopPropagation();
        close();
        hooks.onClose?.();
      });
    });
  }

  function animatedIids(tree: ReplicaNode): Set<string> {
    const s = new Set<string>();
    const walk = (n: ReplicaNode): void => {
      for (const a of n.animations || []) {
        for (const t of a.controlTimelines || []) {
          if (t.animatedControlIID != null) s.add(String(t.animatedControlIID));
        }
      }
      for (const c of n.children || []) walk(c);
    };
    walk(tree);
    return s;
  }

  /** 先空跑一遍布局，量出背景条（CSS Palette_tab）落在 body 内的 y。 */
  function measureBarY(tree: ReplicaNode, w: number, h: number): number {
    let barY: number | null = null;
    const walk = (n: ReplicaNode, px: number, py: number, pw: number, ph: number): void => {
      const r = updatePosition(n, pw, ph);
      const ay = py + r.y;
      const dr = n.drawable;
      if (barY === null && dr && (dr.cssStyles || [])[0] === "Palette_tab") barY = ay;
      for (const c of n.children || []) walk(c, px + r.x, ay, r.w, r.h);
    };
    walk(tree, 0, 0, w, h);
    return barY ?? 0;
  }

  /** 面板里布局数据表达不了的部分：分类名与槽位行由运行时填。
   * 顾问小人 / 统计信息区已在构建期裁掉（build.py）。 */
  const PALETTE_SKIP = new Set(["DNT"]);

  /** 面板布局内部的递归渲染（复用一级菜单那套元素构造）。 */
  function place(node: ReplicaNode, parentEl: HTMLElement, rect: { x: number; y: number; w: number; h: number }, absX: number, absY: number): void {
    if (node.visibility === false) return;
    const el = createNodeElement(node, rect);
    if (animTargets.has(String(node.instanceID))) el.style.opacity = "";
    if (node._module === "Layouts/GlobalUI/CloseButton.js") el.classList.add("hud-close");
    parentEl.appendChild(el);
    if (node._missing || PALETTE_SKIP.has(node.comment ?? "")) return;
    for (const child of node.children || []) {
      const r = updatePosition(child, rect.w, rect.h);
      place(child, el, r, absX + rect.x, absY + rect.y);
    }
  }

  /** 分类名标签：各布局的节点注释不统一（"DNT" / "Category Name DNT" / 无注释），
   * 但文本样式一律是 Header_18_Left，且 text 是占位串 —— 按这两条定位。 */
  function attachLabel(parentEl: HTMLElement, layout: ReplicaNode, label: string): void {
    const node = findLabelNode(layout);
    if (!node) return;
    const el = parentEl.querySelector(`.hud-el[data-iid="${node.instanceID}"]`);
    if (el) el.textContent = label || "";
  }

  function findLabelNode(n: ReplicaNode): ReplicaNode | null {
    if (n.textStyle === "Header_18_Left") return n;
    for (const c of n.children || []) {
      const r = findLabelNode(c);
      if (r) return r;
    }
    return null;
  }

  /** 布局自带工具按钮的分类（见 build.py 的 SKIP_SLOT_ROW）。 */
  const SKIP_SLOT_ROW = new Set(["3193C0BE"]);

  /** 游戏行为：二级槽位一行最多 8 个，超出的用 ◀/▶ 翻页，不换行。 */
  const PAGE_SIZE = 8;

  /* ── hover 信息提示框（游戏里的 BuildingRollover）──
   * 白底圆角窗 + 钢蓝标题 + rollover 大图 + 图底半透明黑条描述 +
   * 锁定项的红色解锁提示。数据全部来自菜单条目 property
   * （0x0A09F5FA 标题 / 0x0A09F5FB 描述 / kPropToolMarqueeImage 大图 /
   *   kPropToolUnlockString 解锁文案）；窗体样式用游戏自带的
   * .BuildingRollover_Background_White，文本用 .toolRollover*。 */
  let rolloverEl: HTMLElement | null = null;
  let rolloverTimer = 0;

  function hideRollover(): void {
    window.clearTimeout(rolloverTimer);
    rolloverEl?.classList.remove("show");
  }

  function showRollover(tool: SlotTool, slot: HTMLElement, bodyEl: HTMLElement): void {
    if (!tool.label && !tool.marquee && !tool.desc) return;
    window.clearTimeout(rolloverTimer);
    rolloverTimer = window.setTimeout(() => {
      if (!rolloverEl) {
        rolloverEl = document.createElement("div");
        rolloverEl.className = "hud-rollover BuildingRollover_Background_White";
      }
      if (rolloverEl.parentElement !== bodyEl) {
        rolloverEl.remove();
        bodyEl.appendChild(rolloverEl);
      }
      const media = tool.marquee || tool.preview;
      rolloverEl.innerHTML = `
        <div class="hud-rollover-title">${tool.label ?? ""}</div>
        <div class="hud-rollover-media${tool.marquee ? "" : " icon-only"}">
          <div class="hud-rollover-img" style="background-image:url('${media ?? ""}')"></div>
          ${tool.desc ? `<div class="hud-rollover-desc toolRolloverDescription">${tool.desc}</div>` : ""}
        </div>
        ${tool.locked && tool.unlock ? `<div class="hud-rollover-unlock toolRolloverUnlockExplanation">${tool.unlock}</div>` : ""}
      `;
      // 定位：槽位的 offsetParent 是槽位行（absolute），先换算到 body 坐标；
      // 水平跟槽位居中并夹在面板内，竖直贴在槽位行上方。
      const row = slot.offsetParent as HTMLElement | null;
      const cx = (row?.offsetLeft ?? 0) + slot.offsetLeft + slot.offsetWidth / 2;
      const top = (row?.offsetTop ?? 0) + slot.offsetTop;
      const width = rolloverEl.offsetWidth || 320;
      const left = Math.max(8, Math.min(cx - width / 2, bodyEl.clientWidth - width - 8));
      rolloverEl.style.left = `${left.toFixed(2)}px`;
      rolloverEl.style.top = `${(top - 8).toFixed(2)}px`;
      rolloverEl.style.transform = "translateY(-100%)";
      rolloverEl.classList.add("show");
    }, 80);
  }

  /** 槽位行：把该分类的工具平铺出来做预览。一行最多 8 个，超出的部分由
   * 面板布局自带的翻页钮（data-comment "page left"/"page right"）翻页查看。
   *
   * 槽位图标 = 菜单条目 property 的 kPropToolIconKey（0x0977AA8F，组 40E02400
   * 的等轴模型渲染 PNG，构建期已拷进 assets/）；preview 为空的条目（如公园
   * 旧数据回填）回退 tool_placeholder.png。 */
  function attachSlotRow(parentEl: HTMLElement, categoryId: string, cfg: ReplicaSlotRow, barY: number): void {
    if (SKIP_SLOT_ROW.has(categoryId)) return; // 布局自带工具按钮的分类不再叠一行
    const tools = hooks.getTools?.(categoryId) ?? (data.tools[categoryId] as SlotTool[] | undefined) ?? [];
    if (!tools.length) return;
    const perRow = Math.min(PAGE_SIZE, tools.length);
    const pageCount = Math.ceil(tools.length / perRow);
    let page = 0;
    // 行首锚在 ◀ 翻页钮右侧 —— 各布局条左端的搜索/信息块宽度不同，
    // 用固定值一定会踩到别的元素。注意 getBoundingClientRect 是缩放后的
    // 客户端坐标，舞台几何按布局坐标系（未缩放）算，必须除回当前缩放比。
    const bodyRect = parentEl.getBoundingClientRect();
    const scaleFactor = bodyRect.width / (parentEl.clientWidth || bodyRect.width || 1) || 1;
    const rel = (el: Element, edge: "left" | "right"): number => {
      const r = el.getBoundingClientRect();
      return ((edge === "right" ? r.right : r.left) - bodyRect.left) / scaleFactor;
    };
    const leftBtns = Array.from(parentEl.querySelectorAll('[data-comment="page left"]'));
    const rightBtns = Array.from(parentEl.querySelectorAll('[data-comment="page right"]'));
    const leftBtn = leftBtns[0];
    const startX = leftBtn ? Math.max(cfg.startX, rel(leftBtn, "right") + 6) : cfg.startX;
    const row = document.createElement("div");
    row.className = "hud-palette-row";
    row.style.left = `${startX}px`;
    row.style.top = `${barY + (cfg.topInBar || 0)}px`; // 相对菜单条，各布局通用
    row.style.width = `${perRow * cfg.pitch}px`;
    row.style.height = `${cfg.slotH}px`;
    parentEl.appendChild(row);

    // 多页时才把布局自带的 ◀/▶ 翻页钮接上（.hud-el 默认 pointer-events:none，
    // 且单页时应与复刻站观感完全一致——装饰态）；端点钮置灰。
    function wirePager(): void {
      if (pageCount <= 1) return;
      const activate = (el: Element): void => {
        (el as HTMLElement).style.pointerEvents = "auto";
        (el as HTMLElement).style.cursor = "pointer";
      };
      const step = (delta: number): void => {
        const next = page + delta;
        if (next < 0 || next >= pageCount) return;
        page = next;
        paintPage();
      };
      leftBtns.forEach((b) => {
        activate(b);
        b.addEventListener("click", (e) => {
          e.stopPropagation();
          step(-1);
        });
      });
      rightBtns.forEach((b) => {
        activate(b);
        b.addEventListener("click", (e) => {
          e.stopPropagation();
          step(1);
        });
      });
      leftBtns.forEach((b) => ((b as HTMLElement).style.opacity = "0.45"));
      rightBtns.forEach((b) => ((b as HTMLElement).style.opacity = page === 0 ? "0.45" : ""));
    }

    function paintPage(): void {
      hideRollover();
      row.textContent = "";
      tools.slice(page * perRow, page * perRow + perRow).forEach((tool, i) => {
        const slot = document.createElement("div");
        slot.className = "hud-slot";
        if (tool.locked) slot.classList.add("locked");
        slot.style.left = `${(i * cfg.pitch).toFixed(2)}px`;
        slot.style.top = "0px";
        slot.style.width = `${(cfg.pitch - 4).toFixed(2)}px`;
        slot.style.height = `${(cfg.slotH - 8).toFixed(2)}px`;
        const img = tool.preview || tool.marquee || "/game-ui/replica/assets/tool_placeholder.png";
        slot.style.backgroundImage = `url('${img}')`;
        slot.title = tool.label || tool.instance;
        slot.addEventListener("mouseenter", () => showRollover(tool, slot, parentEl));
        slot.addEventListener("mouseleave", hideRollover);
        slot.addEventListener("click", () => hooks.onSlotClick?.(categoryId, tool));
        row.appendChild(slot);
        if (tool.isNew) {
          const del = document.createElement("span");
          del.className = "wb-del-x";
          del.title = "删除此条目";
          del.addEventListener("click", (e) => {
            e.stopPropagation();
            hooks.onRemoveItem?.(categoryId, tool);
          });
          slot.appendChild(del);
        }
      });
      // 端点翻页钮置灰（仅多页时）
      if (pageCount > 1) {
        leftBtns.forEach((b) => ((b as HTMLElement).style.opacity = page > 0 ? "" : "0.45"));
        rightBtns.forEach((b) =>
          ((b as HTMLElement).style.opacity = page < pageCount - 1 ? "" : "0.45"),
        );
      }
    }

    wirePager();
    paintPage();
  }

  function open(categoryId: string, label: string): boolean {
    if (!host) return false;
    if (currentId === categoryId) return true;
    clear();
    if (!paint(categoryId, label)) return false;
    currentId = categoryId;
    currentLabel = label;
    void host.offsetWidth; // 强制回流，保证过渡从初始态开始
    host.classList.add("open");
    return true;
  }

  function repaint(): void {
    if (!host || currentId === null || !host.classList.contains("open")) return;
    const id = currentId;
    const label = currentLabel;
    body?.remove();
    body = null;
    // host 保持 .open：重建的 body 直接处于终态（不重播升起动画）
    if (paint(id, label)) currentId = id;
    else currentId = null;
  }

  function close(): void {
    if (!host || currentId === null) return;
    host.classList.remove("open");
    currentId = null;
  }

  return {
    init,
    open,
    close,
    repaint,
    isOpen: (categoryId) => currentId === categoryId,
    current: () => currentId,
  };
}
