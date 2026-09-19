<script setup lang="ts">
/**
 * UI 工作台舞台：在 Shadow DOM 里按 ui_replica 复刻站（probe_fire/ui_replica）
 * 逐像素重建 1600×900 的游戏 HUD——布局树经 scrui 两段式布局解析成嵌套绝对定位
 * DOM，样式/数据/几何全部来自复刻站的逆向产物（css/*.css + data/*.js），
 * 默认观感与复刻站完全一致。
 *
 * 舞台之上只叠工作台自身的元素：一级菜单末位的「＋」与自定义分类删除角标
 * （编辑功能入口）。菜单交互（点圆钮开合二级面板、点槽位进编辑）桥接到
 * uiWorkbench store。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import { injectReplicaStyles } from "@/lib/game-ui/replica/styles";
import { createHudRender } from "@/lib/game-ui/replica/render";
import { createHudPalette, type HudPalette, type SlotTool } from "@/lib/game-ui/replica/palette";
import type { ReplicaData } from "@/lib/game-ui/replica/types";

const { t } = useI18n();
const store = useUiWorkbenchStore();
const { customCategories } = storeToRefs(store);

/* ── 舞台缩放：容器内等比放下 1600×900（舞台高度固定，不随面板高度变化） ── */
const host = ref<HTMLElement>();
const stageInner = ref<HTMLElement>();
const scale = ref(1);
let observer: ResizeObserver | undefined;
onMounted(() => {
  observer = new ResizeObserver((entries) => {
    const rect = entries[0]?.contentRect;
    if (rect?.width) {
      scale.value = Math.min(rect.width / 1600, rect.height / 900);
    }
  });
  if (host.value) observer.observe(host.value);
});
onBeforeUnmount(() => observer?.disconnect());

/* ── 复刻渲染装配（Shadow DOM，样式隔离，游戏 CSS 不泄漏到应用） ── */
const booting = ref(false);
const bootFailed = ref(false);
let shadow: ShadowRoot | null = null;
let palette: HudPalette | null = null;
/** 分类圆钮行锚点（拊件定位用）。 */
const rowGeom = ref<{ absY: number; count: number } | null>(null);

const builtinCount = computed(() => store.replica?.model.categories.length ?? 0);

watch(
  () => store.replica,
  async (data) => {
    if (data) await boot(data);
  },
);

/** 一级菜单里的新增分类/条目变化需要重建行或重绘槽位。 */
watch(
  () => customCategories.value.length,
  async () => {
    if (store.replica) await boot(store.replica);
  },
);
watch(
  () => store.activeMenu?.entries.map((entry) => `${entry.tool.id}:${entry.tool.label}`).join("|"),
  () => palette?.repaint(),
);

async function boot(data: ReplicaData): Promise<void> {
  if (!stageInner.value || booting.value) return;
  booting.value = true;
  bootFailed.value = false;
  try {
    shadow ??= stageInner.value.attachShadow({ mode: "open" });
    await injectReplicaStyles(shadow);
    injectWorkbenchStyles(shadow);

    // 重建视口（复刻站 index.html 的 #viewport > #stage 结构；缩放由外层 Vue 负责，
    // 这里按复刻站 hud.js fit() 的 k=1 摆放 #viewport）。
    shadow.querySelectorAll("#viewport").forEach((el) => el.remove());
    const viewport = document.createElement("div");
    viewport.id = "viewport";
    viewport.style.transform = "translate(-50%, -50%) scale(1)";
    const stageEl = document.createElement("div");
    stageEl.id = "stage";
    viewport.appendChild(stageEl);
    shadow.appendChild(viewport);

    const hud = createHudRender(data, {
      extraCategories: customCategories.value.map((c) => ({
        id: c.id,
        label: c.label,
        icon: c.icon,
        iconHash: null,
      })),
    });
    const { categoryRow } = hud.render(stageEl);
    rowGeom.value = categoryRow;

    palette = createHudPalette(data, hud.createNodeElement, {
      getTools: (categoryId) => effectiveTools(data, categoryId),
      onSlotClick: (categoryId, tool) => store.openEditor(categoryId, tool.instance),
      onRemoveItem: (categoryId, tool) => store.removeItem(categoryId, tool.instance),
      onClose: () => {
        clearSelection();
        store.leaveMenu();
      },
    });
    palette.init(stageEl);
    wireCategoryButtons();
  } catch (cause) {
    bootFailed.value = true;
    console.warn("[uiWorkbench] 复刻舞台装配失败", cause);
  } finally {
    booting.value = false;
  }
}

/** 面板打开时若该分类正在编辑，槽位行展示「基础 + 新增」的最终视图；
 * 新增条目没有游戏预览资源 → palette 层回退 tool_placeholder.png。 */
function effectiveTools(data: ReplicaData, categoryId: string): SlotTool[] | undefined {
  const activeMenu = store.activeMenu;
  if (!activeMenu || activeMenu.id !== categoryId) return undefined;
  const base = data.tools[categoryId] ?? [];
  return activeMenu.entries.map((entry) => {
    const source = base.find((tool) => tool.instance === entry.tool.id);
    // 编辑覆盖（activeMenu 的 tool 已应用 edits）优先于基础数据：
    // 舞台槽位与弹窗实时反映名称/描述/造价/图片的修改。
    const edited = entry.tool;
    return {
      instance: entry.tool.id,
      label: edited.label || source?.label || "",
      preview: edited.preview ?? source?.preview ?? null,
      marquee: edited.marquee ?? source?.marquee ?? null,
      desc: edited.desc ?? source?.desc,
      unlock: edited.unlock ?? source?.unlock,
      stats: source?.stats,
      cost: edited.cost ?? source?.cost ?? null,
      upkeep: edited.upkeep ?? source?.upkeep ?? null,
      locked: edited.locked ?? source?.locked ?? false,
      isNew: entry.isNew,
    };
  });
}

/** 复刻站 hud.js wireCategoryButtons 的移植 + store 同步：
 * 选中外观由 CategoryButtonWithIcon2 自带的 ButtonSelected 动画决定
 * （即 #6 "toggle button2" 与 #8 "ring on"，对应 layer-selected）。 */
function wireCategoryButtons(): void {
  if (!shadow) return;
  shadow.querySelectorAll<HTMLElement>(".cat-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const id = btn.dataset.category;
      if (!id || !palette) return;
      const wasOpen = palette.isOpen(id);
      clearSelection();
      if (wasOpen) {
        palette.close();
        store.leaveMenu();
      } else {
        btn.classList.add("layer-selected");
        palette.open(id, btn.title);
        store.enterMenu(id);
      }
    });
  });
}

function clearSelection(): void {
  shadow?.querySelectorAll(".cat-btn.layer-selected").forEach((b) => b.classList.remove("layer-selected"));
}

/** 工作台拊件的补充样式（注入 ShadowRoot；只作用于拊件，不碰游戏 UI）。 */
function injectWorkbenchStyles(root: ShadowRoot): void {
  const style = document.createElement("style");
  style.textContent = `
.wb-del-x {
  align-items: center;
  background: #d3242a;
  border: 1px solid #fff;
  border-radius: 50%;
  box-sizing: border-box;
  color: #fff;
  cursor: pointer;
  display: flex;
  font-size: 11px;
  height: 15px;
  justify-content: center;
  line-height: 1;
  position: absolute;
  width: 15px;
  z-index: 10;
}
`;
  root.appendChild(style);
}

/* ── 一级菜单拊件几何：圆钮行 left(i) = centerX - count*pitch/2 + i*pitch ── */
function rowAnchorStyle(index: number): Record<string, string> {
  const geom = rowGeom.value;
  const row = store.replica?.model.row;
  if (!geom || !row) return { visibility: "hidden" };
  return {
    left: `${(row.centerX - (geom.count * row.pitch) / 2 + index * row.pitch).toFixed(2)}px`,
    top: `${(geom.absY + (row.offsetY || 0)).toFixed(2)}px`,
  };
}

/** 自定义分类删除角标：锚在对应圆钮（43×62）右上角。 */
function catDelStyle(index: number): Record<string, string> {
  const anchor = rowAnchorStyle(index);
  return {
    left: `calc(${anchor.left} + 30px)`,
    top: `calc(${anchor.top} - 4px)`,
  };
}

/** 新增分类「＋」：43×43 圆，在 62px 行单元里垂直居中。 */
function addCatStyle(): Record<string, string> {
  const anchor = rowAnchorStyle(rowGeom.value?.count ?? 0);
  return {
    left: anchor.left,
    top: `calc(${anchor.top} + 9px)`,
  };
}
</script>

<template>
  <div ref="host" class="stage-host">
    <div class="stage" :style="{ width: `${1600 * scale}px`, height: `${900 * scale}px` }">
      <div ref="stageInner" class="stage-inner" :style="{ transform: `scale(${scale})` }" />

      <!-- 一级菜单拊件：自定义分类删除角标 + 末位新增分类 -->
      <template v-if="rowGeom">
        <span
          v-for="(cat, index) in customCategories"
          :key="cat.id"
          class="wb-del-x"
          role="button"
          :title="t('studio.workbench.removeCategory')"
          :style="catDelStyle(builtinCount + index)"
          @click.stop="store.removeCategory(cat.id)"
          >×</span
        >
        <button
          type="button"
          class="wb-add-cat"
          title="新增一级菜单分类"
          :style="addCatStyle()"
          @click="store.addCategory('新分类')"
        >
          <span aria-hidden="true">＋</span>
        </button>
      </template>

      <p v-if="store.loadError" class="stage-error" role="alert">
        复刻数据装载失败：{{ store.loadError }}
      </p>
    </div>
  </div>
</template>

<style scoped>
.stage-host {
  align-items: center;
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  display: flex;
  height: 100%;
  justify-content: center;
  min-height: 0;
  overflow: hidden;
  width: 100%;
}
.stage {
  flex: none;
  position: relative;
}
/* Shadow 宿主：继承起点钉在浏览器默认值上，保证舞台内文字与复刻站一致
 * （应用主题的字体/前景色/行高经 Tailwind Preflight 设置在 html 上，
 * 会穿过 shadow 边界继承进来，必须在宿主拦下）。 */
.stage-inner {
  color: #000;
  font-family: sans-serif;
  font-size: 16px;
  font-style: normal;
  font-variant: normal;
  font-weight: 400;
  letter-spacing: normal;
  line-height: normal;
  text-rendering: auto;
  text-transform: none;
  height: 900px;
  overflow: hidden;
  position: relative;
  transform-origin: top left;
  width: 1600px;
}

/* ── 工作台拊件（一级菜单末位「＋」/ 自定义分类角标） ── */
.wb-del-x {
  align-items: center;
  background: #d3242a;
  border: 1px solid #fff;
  border-radius: 50%;
  color: #fff;
  cursor: pointer;
  display: flex;
  font-size: 11px;
  height: 15px;
  justify-content: center;
  line-height: 1;
  position: absolute;
  width: 15px;
  z-index: 3;
}
.wb-add-cat {
  align-items: center;
  background: transparent;
  border: 1.5px dashed rgb(200 214 228 / 85%);
  border-radius: 50%;
  box-sizing: border-box;
  color: rgb(235 242 248 / 90%);
  cursor: pointer;
  display: flex;
  height: 43px;
  justify-content: center;
  padding: 0;
  position: absolute;
  width: 43px;
}
.wb-add-cat:hover {
  border-color: rgb(8 120 254 / 90%);
  color: #fff;
}

.stage-error {
  background: rgb(70 20 20 / 85%);
  border-radius: 6px;
  color: #ffd9d9;
  font-size: 12px;
  left: 50%;
  padding: 6px 12px;
  position: absolute;
  top: 12px;
  transform: translateX(-50%);
  z-index: 5;
}
</style>
