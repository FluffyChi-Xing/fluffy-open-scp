<script setup lang="ts">
/**
 * UI 工作台舞台：用游戏原生资产 + scrui 布局公式等比例重建 1600×900 的游戏 HUD。
 *
 * 两个层次：
 * 1. **美术层**（数据驱动）：globalui2 布局树经 scrui 两段式布局（设计尺寸
 *    Init 偏移 → 视口尺寸 Update 级联）解析出每个 drawable 的绝对矩形，
 *    全部按引擎自己的数学摆放；
 * 2. **交互层**（本工作台）：菜单按钮/建筑槽位/新增占位块锚定到布局矩形上，
 *    点击 → 右侧面板预览该菜单级 → Sheet 单条编辑 → 实时生效。
 *
 * 图标统一 FIcon（找不到的语义用占位 Box）；底栏读数为占位值；
 * 参考截图可 0–100% 叠加校准（public/game-ui/reference/*.png）。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import { CATEGORY_ICONS, toolIconName } from "@/lib/game-ui/workbench";

const { t } = useI18n();
const store = useUiWorkbenchStore();
const {
  dimension,
  data,
  activeMenu,
  categories,
  selectedMenuId,
  entered,
  hudImages,
  page,
  pageCount,
  pagedEntries,
} = storeToRefs(store);
// 函数不走 storeToRefs（其只转状态/getter）
const setPage = store.setPage;

/* ── 舞台缩放：容器内等比放下 1600×900（舞台高度固定，不随面板高度变化） ── */
const host = ref<HTMLElement>();
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

const props = defineProps<{
  /** 参考截图叠加透明度（0 = 关闭）。 */
  overlayOpacity: number;
}>();

const overlaySrc = computed(
  () => `/game-ui/reference/${entered.value ? "university" : "city"}.png`,
);

/* ── 布局矩形锚点（instanceID 来自 globalui2.json） ── */
function rectStyle(id: string): Record<string, string> {
  const r = store.rect(id);
  if (!r) return { visibility: "hidden" as const };
  return {
    left: `${r.x}px`,
    top: `${r.y}px`,
    width: `${r.w}px`,
    height: `${r.h}px`,
  };
}

/* ── 底栏占位读数（空白原件 + 静态数值） ── */
const placeholders = {
  time: "8:41 PM",
  cityName: "御木林",
  money: "§422,668",
  income: "+5,632 / 小時",
  population: "54,666",
};

/** 大学槽位的占位计数（对齐参考截图：第 3/4 槽位）。 */
function slotCounter(index: number): string {
  return index === 2 ? "0 / 3" : index === 3 ? "0 / 0" : "";
}

/** 满意度笑脸：锚定 Mayor Rating 精灵图节点（1888），裁最右绿脸。 */
const smileyStyle = computed(() => {
  const r = store.rect("1888");
  if (!r) return { visibility: "hidden" as const };
  return { left: `${r.x + 105}px`, top: `${r.y + 1}px` };
});

/** 右上角系统按钮（原生图标缺失，FIcon 占位）。 */
const topButtons = computed(() => [
  { id: "1079", icon: "Mail", title: t("studio.workbench.topInvites") },
  { id: "1364", icon: "Star", title: t("studio.workbench.topAchievements") },
  { id: "1367", icon: "Crosshair", title: t("studio.workbench.topChallenges") },
  { id: "1273", icon: "ChartBar", title: t("studio.workbench.topLeaderboards") },
]);

function isCustomCategory(id: string): boolean {
  return id.startsWith("NEW-CAT-");
}

function removeCategoryAt(id: string): void {
  store.removeCategory(id);
}
</script>

<template>
  <div ref="host" class="stage-host">
    <div class="stage" :style="{ width: `${1600 * scale}px`, height: `${900 * scale}px` }">
      <div class="stage-inner" :style="{ transform: `scale(${scale})` }">
        <!-- 世界底色（工作台不加载 3D 场景，用中性地平线示意） -->
        <div class="backdrop" aria-hidden="true">
          <span class="backdrop-label">3D 城市视口（示意）</span>
        </div>

        <!-- 美术层：scrui 布局公式定位的游戏原生资产（加载失败自动隐藏） -->
        <img
          v-for="(img, index) in hudImages"
          :key="`art-${index}`"
          class="art"
          :src="img.src"
          :style="{
            left: `${img.x}px`,
            top: `${img.y}px`,
            width: `${img.w}px`,
            height: `${img.h}px`,
          }"
          alt=""
          @error="($event.target as HTMLImageElement).style.visibility = 'hidden'"
        />

        <!-- 顶部：城市通知条 / 二级菜单标题 -->
        <div v-if="!entered" class="ticker" :style="rectStyle('569')">
          <span class="ticker-icon" aria-hidden="true" />
          <span>模擬城市伺服器連線中，正在嘗試重連。</span>
        </div>
        <div v-else class="screen-title">{{ activeMenu?.label }}</div>

        <!-- 右上角：社交/系统按钮（原生图标缺失 → FIcon 占位） -->
        <button
          v-for="btn in topButtons"
          :key="btn.id"
          type="button"
          class="top-btn"
          :style="rectStyle(btn.id)"
          :title="btn.title"
        >
          <FIcon :name="btn.icon" :size="15" aria-label="" />
        </button>
        <button type="button" class="main-menu" :style="rectStyle('46')" aria-label="主菜单">
          ···
        </button>

        <!-- 左侧维度切换：点击目标覆盖在美术层簇上，选中显示品牌色环 -->
        <button
          type="button"
          class="dim-tab"
          :class="{ active: dimension === 'city' }"
          :style="rectStyle('772')"
          :title="t('studio.workbench.dimCity')"
          @click="store.selectDimension('city')"
        >
          <span class="dim-circle big"><FIcon name="House" :size="24" aria-label="" /></span>
        </button>
        <button
          type="button"
          class="dim-tab"
          :class="{ active: dimension === 'bigbiz' }"
          :style="rectStyle('1459')"
          :title="t('studio.workbench.dimBigbiz')"
          @click="store.selectDimension('bigbiz')"
        >
          <span class="dim-circle"><FIcon name="UsersRound" :size="18" aria-label="" /></span>
        </button>
        <button
          type="button"
          class="dim-tab"
          :class="{ active: dimension === 'region' }"
          :style="rectStyle('119')"
          :title="t('studio.workbench.dimRegion')"
          @click="store.selectDimension('region')"
        >
          <span class="dim-circle"><FIcon name="Globe" :size="16" aria-label="" /></span>
        </button>

        <!-- 城市主菜单：一级分类（悬浮圆钮）⇄ 二级槽位面板，滑动过渡 -->
        <Transition name="lv" mode="out-in">
          <div v-if="!entered" key="l1" class="tool-viewport" :style="rectStyle('766')">
            <div class="tool-row">
              <button
                v-for="category in categories"
                :key="category.id"
                type="button"
                class="tool-button"
                :title="`${category.label}（${category.items.length}）`"
                @click="store.enterMenu(category.id)"
              >
                <FIcon
                  class="tool-icon"
                  :name="CATEGORY_ICONS[category.id] ?? 'Box'"
                  :size="22"
                  aria-label=""
                />
                <span
                  v-if="isCustomCategory(category.id)"
                  class="del-x"
                  role="button"
                  :title="t('studio.workbench.removeCategory')"
                  @click.stop="removeCategoryAt(category.id)"
                  >×</span
                >
              </button>
              <!-- 一级菜单末位：新增分类占位 -->
              <button
                type="button"
                class="tool-button add"
                title="新增一级菜单分类"
                @click="store.addCategory('新分类')"
              >
                <span aria-hidden="true">＋</span>
              </button>
            </div>
          </div>
          <div v-else key="l2" class="palette-strip">
            <div v-if="pageCount > 1" class="page-ctrl">
              <button
                type="button"
                class="page-btn"
                :disabled="page <= 1"
                aria-label="上一页"
                @click="setPage(page - 1)"
              >
                ‹
              </button>
              <span class="tabnum">{{ page }} / {{ pageCount }}</span>
              <button
                type="button"
                class="page-btn"
                :disabled="page >= pageCount"
                aria-label="下一页"
                @click="setPage(page + 1)"
              >
                ›
              </button>
            </div>
            <div class="palette-slots">
              <button
                v-for="(entry, index) in pagedEntries"
                :key="entry.tool.id"
                type="button"
                class="slot"
                :class="{ selected: index === 1 }"
                :title="entry.tool.label"
                @click="store.openEditor(activeMenu!.id, entry.tool.id)"
              >
                <span class="slot-frame" aria-hidden="true" />
                <FIcon
                  class="slot-icon"
                  :name="toolIconName(entry.tool)"
                  :size="30"
                  aria-label=""
                />
                <span class="slot-label">{{ entry.tool.label }}</span>
                <span v-if="slotCounter(index)" class="slot-counter tabnum">{{
                  slotCounter(index)
                }}</span>
                <span
                  v-if="entry.isNew"
                  class="del-x"
                  role="button"
                  :title="t('studio.workbench.removeEntry')"
                  @click.stop="store.removeItem(activeMenu!.id, entry.tool.id)"
                  >×</span
                >
              </button>
              <!-- 二级菜单末位：新增条目占位 -->
              <button
                type="button"
                class="slot add"
                title="新增二级菜单条目"
                @click="activeMenu && store.addItem(activeMenu.id, '新条目', null)"
              >
                <span aria-hidden="true">＋</span>
              </button>
            </div>
          </div>
        </Transition>

        <!-- 二级：分类专属侧件（道路=形状工具；教育=学位面板） -->
        <div v-if="entered && selectedMenuId === 'road'" class="road-tools">
          <div class="road-shapes" aria-hidden="true">
            <span class="shape" /><span class="shape" /><span class="shape" /><span
              class="shape"
            /><span class="shape" />
          </div>
          <label class="guide-row">
            <input type="checkbox" checked disabled />
            <span>指南</span>
          </label>
        </div>

        <div v-if="entered && selectedMenuId === 'education'" class="edu-panel">
          <header class="edu-head">
            <span>教育</span>
            <button type="button" class="edu-close" aria-label="关闭">×</button>
          </header>
          <div class="edu-body">
            <div class="edu-number tabnum">2,032<span class="edu-sub">/ 2,342</span></div>
            <div class="edu-caption">入学人数</div>
            <div class="edu-row"><span>教育程度：</span><span class="edu-chips" /></div>
            <div class="edu-row"><span>科技等级</span><span class="edu-chips" /></div>
            <div class="edu-row"><span>去：</span><span class="edu-bar" /></div>
          </div>
        </div>

        <!-- 底栏（美术层提供衬带/tab 板/RCI/图层钮），这里只叠加读数与交互 -->
        <div class="stats-overlay">
          <button type="button" class="play" aria-label="暂停 / 继续">
            <span aria-hidden="true">▶</span>
          </button>
          <span class="clock tabnum">{{ placeholders.time }}</span>
          <span class="speed tabnum" aria-hidden="true">▶▶|</span>
          <span class="name-text">{{ placeholders.cityName }}</span>
          <span class="smiley" :style="smileyStyle" aria-hidden="true">
            <img v-if="data?.assets.mayorRating" :src="data.assets.mayorRating" alt="" />
          </span>
          <span class="money tabnum">{{ placeholders.money }}</span>
          <span class="income tabnum">{{ placeholders.income }}</span>
          <span class="pop tabnum">
            <FIcon name="Users" :size="15" aria-label="" />
            {{ placeholders.population }}
          </span>
        </div>

        <!-- 参考截图叠加（校准模式） -->
        <img
          v-if="props.overlayOpacity > 0"
          class="reference-overlay"
          :src="overlaySrc"
          :style="{ opacity: props.overlayOpacity }"
          alt="游戏内参考截图"
        />
      </div>
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
.stage-inner {
  height: 900px;
  overflow: hidden;
  position: relative;
  transform-origin: top left;
  width: 1600px;
}
.backdrop {
  background:
    linear-gradient(180deg, #243a52 0%, #35506b 46%, #4c6b52 70%, #3c5643 100%);
  inset: 0;
  position: absolute;
}
.backdrop-label {
  color: rgb(255 255 255 / 30%);
  font-size: 14px;
  left: 50%;
  letter-spacing: 0.12em;
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
}
.art {
  position: absolute;
}
.tabnum {
  font-variant-numeric: tabular-nums;
}

/* ── 顶部 ── */
.ticker {
  align-items: flex-start;
  display: flex;
  gap: 8px;
  position: absolute;
}
.ticker-icon {
  background: #d2202a;
  border: 2px solid #fff;
  border-radius: 3px;
  box-shadow: 0 1px 4px rgb(9 20 34 / 40%);
  flex: none;
  height: 26px;
  width: 26px;
}
.ticker > span:last-child {
  background: linear-gradient(180deg, rgb(250 251 253 / 92%), rgb(226 233 240 / 92%));
  border: 1px solid rgb(210 40 40 / 65%);
  border-radius: 4px;
  color: #b3261e;
  font-size: 13px;
  padding: 6px 14px;
}
.screen-title {
  background: linear-gradient(180deg, rgb(252 253 255 / 94%), rgb(228 235 242 / 94%));
  border-radius: 4px;
  box-shadow: 0 1px 4px rgb(9 20 34 / 35%);
  color: #223c5c;
  font-size: 17px;
  font-weight: 700;
  left: 50%;
  letter-spacing: 0.35em;
  padding: 5px 26px 5px 32px;
  position: absolute;
  top: 6px;
  transform: translateX(-50%);
}
.main-menu {
  background: linear-gradient(180deg, rgb(252 253 255 / 94%), rgb(228 235 242 / 94%));
  border: 0;
  border-radius: 6px;
  color: #223c5c;
  cursor: pointer;
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.1em;
  position: absolute;
}

/* ── 左侧维度切换簇 ── */
.mode-cluster {
  align-items: center;
  display: flex;
  flex-direction: column;
  gap: 8px;
  position: absolute;
}
.dim-tab {
  background: transparent;
  border: 0;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  position: absolute;
}
.dim-circle {
  align-items: center;
  background: radial-gradient(circle at 50% 34%, #fbfdff 96%, #d2deea);
  border: 2px solid rgb(245 248 251 / 95%);
  border-radius: 50%;
  box-shadow: 0 2px 6px rgb(9 20 34 / 40%);
  color: #2c4a6e;
  display: flex;
  height: 44px;
  justify-content: center;
  transition: box-shadow 140ms ease;
  width: 44px;
}
.dim-circle.big {
  height: 60px;
  width: 60px;
}
.dim-tab.active .dim-circle {
  background: radial-gradient(circle at 50% 30%, #6cb8f2 0%, #2f86d6 55%, #1c5fa8 100%);
  border-color: rgb(255 255 255 / 96%);
  color: #fff;
  box-shadow: 0 0 14px rgb(8 120 254 / 55%);
}
.page-ctrl {
  align-items: center;
  color: #eaf2fa;
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  position: absolute;
  right: 6px;
  top: -22px;
}
.page-btn {
  background: rgb(250 252 254 / 92%);
  border: 1px solid rgb(160 178 196 / 80%);
  border-radius: 4px;
  color: #223c5c;
  cursor: pointer;
  font-size: 14px;
  height: 22px;
  line-height: 1;
  width: 22px;
}
.page-btn:disabled {
  cursor: default;
  opacity: 0.4;
}
.lv-enter-active,
.lv-leave-active {
  transition: opacity 240ms cubic-bezier(0.2, 0, 0, 1), translate 240ms cubic-bezier(0.2, 0, 0, 1);
}
.lv-enter-from {
  opacity: 0;
  translate: 0 26px;
}
.lv-leave-to {
  opacity: 0;
  translate: 0 -18px;
}

/* ── 城市分类：悬浮圆钮（无底层衬卡）+ 一二级滑动过渡 ── */
.tool-viewport {
  overflow: hidden;
  position: absolute;
}
.tool-track {
  display: flex;
  height: 100%;
  justify-content: center;
  transition: translate 320ms cubic-bezier(0.2, 0, 0, 1);
  width: 200%;
}
.tool-track.entered {
  translate: -50% 0;
}
.tool-row {
  align-items: center;
  display: flex;
  flex: none;
  gap: 7px;
  justify-content: center;
  width: 50%;
}
.tool-row.level2 {
  justify-content: flex-start;
  overflow-x: auto;
  padding: 4px 2px;
  scrollbar-width: thin;
}
.tool-button {
  align-items: center;
  background: radial-gradient(circle at 50% 32%, rgb(252 253 255 / 97%), rgb(214 226 238 / 92%));
  border: 0;
  border-radius: 50%;
  box-shadow: 0 3px 6px rgb(9 20 34 / 45%), inset 0 -2px 4px rgb(120 145 170 / 35%);
  color: #2c4a6e;
  cursor: pointer;
  display: flex;
  flex: none;
  height: 52px;
  justify-content: center;
  padding: 0;
  position: relative;
  transition: box-shadow 140ms ease, color 140ms ease;
  width: 52px;
}
.tool-button:hover {
  color: #0878fe;
}
.tool-button.selected {
  box-shadow: 0 0 0 2.5px rgb(8 120 254 / 90%), 0 0 16px rgb(8 120 254 / 55%),
    inset 0 -2px 4px rgb(120 145 170 / 35%);
  color: #0878fe;
}
.tool-button.back {
  margin-inline-end: 10px;
}
.tool-button.add {
  border: 1.5px dashed rgb(44 74 110 / 65%);
  box-shadow: none;
  color: #2c4a6e;
  font-size: 20px;
}
.tool-button.add:hover {
  color: #0878fe;
  border-color: #0878fe;
}
.del-x {
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
  right: -3px;
  top: -3px;
  width: 15px;
}

/* ── 大学建筑槽位条（游戏内此层带浅色衬带） ── */
.palette-strip {
  height: 130px;
  left: 170px;
  position: absolute;
  top: 722px;
  width: 1090px;
}
.palette-slots {
  display: flex;
  gap: 6px;
  justify-content: center;
}
.slot {
  background: rgb(20 30 46 / 18%);
  border: 0;
  cursor: pointer;
  height: 82px;
  padding: 4px 4px 0;
  position: relative;
  width: 116px;
}
.slot.selected {
  outline: 2px solid rgb(8 120 254 / 90%);
  outline-offset: -2px;
}
.slot-frame {
  background: linear-gradient(180deg, rgb(250 252 254 / 24%), rgb(210 224 238 / 30%));
  border-radius: 6px;
  inset: 0;
  position: absolute;
}
.slot-icon {
  color: #eaf2fa;
  left: 50%;
  position: absolute;
  text-shadow: 0 1px 3px rgb(9 20 34 / 45%);
  top: 22px;
  transform: translateX(-50%);
}
.slot-label {
  bottom: 2px;
  color: #fff;
  font-size: 11px;
  left: 50%;
  position: absolute;
  text-shadow: 0 1px 2px rgb(0 0 0 / 65%);
  transform: translateX(-50%);
  white-space: nowrap;
}
.slot-counter {
  background: #0878fe;
  border-radius: 9px;
  bottom: -9px;
  color: #fff;
  font-size: 10px;
  left: 50%;
  padding: 1px 7px;
  position: absolute;
  transform: translateX(-50%);
  white-space: nowrap;
}
.slot.add {
  align-items: center;
  border: 1.5px dashed rgb(240 246 252 / 70%);
  border-radius: 8px;
  color: rgb(240 246 252 / 85%);
  display: flex;
  font-size: 22px;
  justify-content: center;
}
.decline-row {
  display: flex;
  gap: 6px;
  margin-top: 8px;
  padding-inline-start: 8px;
}
.decline-cell {
  height: 30px;
  position: relative;
  width: 116px;
}
.decline-btn {
  height: 28px;
  left: 8px;
  position: absolute;
  width: 100px;
}
.decline-text {
  color: #8a2b20;
  font-size: 12px;
  font-weight: 700;
  left: 50%;
  position: absolute;
  top: 6px;
  transform: translateX(-50%);
}

/* ── 大学左侧道路工具 ── */
.road-tools {
  left: 10px;
  position: absolute;
  top: 762px;
  width: 148px;
}
.road-shapes {
  background: linear-gradient(180deg, rgb(250 251 253 / 92%), rgb(228 235 242 / 92%));
  border-radius: 8px;
  display: flex;
  gap: 4px;
  padding: 6px;
}
.shape {
  border: 1.5px solid #35506b;
  border-radius: 3px;
  flex: 1;
  height: 22px;
}
.guide-row {
  align-items: center;
  color: #f2f6fa;
  display: flex;
  font-size: 12px;
  gap: 6px;
  margin-top: 6px;
  text-shadow: 0 1px 2px rgb(0 0 0 / 60%);
}

/* ── 大学右侧教育面板 ── */
.edu-panel {
  background: linear-gradient(180deg, rgb(252 253 255 / 96%), rgb(236 241 246 / 96%));
  border: 1px solid rgb(160 178 196 / 90%);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgb(9 20 34 / 30%);
  color: #223c5c;
  height: 148px;
  position: absolute;
  right: 10px;
  top: 646px;
  width: 264px;
}
.edu-head {
  align-items: center;
  border-bottom: 1px solid rgb(160 178 196 / 60%);
  display: flex;
  font-size: 13px;
  font-weight: 700;
  justify-content: space-between;
  padding: 5px 10px;
}
.edu-close {
  background: transparent;
  border: 0;
  color: #b3261e;
  cursor: pointer;
  font-size: 14px;
}
.edu-body {
  font-size: 11.5px;
  padding: 6px 10px;
}
.edu-number {
  font-size: 21px;
  font-weight: 700;
}
.edu-sub {
  color: #64788e;
  font-size: 11px;
  font-weight: 400;
  margin-inline-start: 4px;
}
.edu-caption {
  color: #b8860b;
  font-size: 10.5px;
  margin-bottom: 4px;
}
.edu-row {
  align-items: center;
  display: flex;
  gap: 6px;
  margin-top: 3px;
}
.edu-chips::before {
  content: "❀ ❀ ❀ ✿ ✿";
  color: #4c9a52;
  letter-spacing: 2px;
}
.edu-bar {
  background: rgb(120 140 160 / 30%);
  border-radius: 3px;
  flex: 1;
  height: 8px;
}

/* ── 底栏读数叠加（衬带/tab 板/RCI/图层钮由美术层 scrui 布局提供） ── */
.stats-overlay {
  height: 52px;
  inset-inline: 0;
  bottom: 0;
  position: absolute;
}
.stats-overlay::before {
  content: "";
  background: linear-gradient(180deg, rgb(240 245 250 / 88%), rgb(214 226 238 / 94%));
  border-top: 1px solid rgb(255 255 255 / 70%);
  box-shadow: 0 -2px 8px rgb(9 20 34 / 25%);
  height: 52px;
  inset-inline: 0;
  bottom: 0;
  position: absolute;
}
.play {
  background: linear-gradient(180deg, #d3242a, #a91018);
  border: 1px solid #7e0c12;
  border-radius: 6px;
  color: #fff;
  cursor: pointer;
  font-size: 13px;
  height: 36px;
  left: 10px;
  position: absolute;
  top: 8px;
  width: 36px;
}
.clock {
  color: #1d2f4a;
  font-size: 15px;
  font-weight: 700;
  left: 58px;
  position: absolute;
  top: 17px;
}
.speed {
  color: #2c3e54;
  font-size: 12px;
  left: 142px;
  letter-spacing: 1px;
  position: absolute;
  top: 20px;
}
.name-text {
  color: #1d2f4a;
  font-size: 13.5px;
  font-weight: 600;
  left: 163px;
  position: absolute;
  top: 19px;
}
.money {
  color: #1d2f4a;
  font-size: 16px;
  font-weight: 700;
  left: 490px;
  position: absolute;
  top: 16px;
}
.income {
  color: #2f9e44;
  font-size: 12px;
  font-weight: 600;
  left: 600px;
  position: absolute;
  top: 20px;
}
.pop {
  align-items: center;
  color: #2f6fd0;
  display: flex;
  font-size: 16px;
  font-weight: 700;
  gap: 6px;
  left: 712px;
  position: absolute;
  top: 16px;
}
.smiley {
  border-radius: 50%;
  display: inline-block;
  height: 26px;
  overflow: hidden;
  width: 26px;
}
/* mayorRating 精灵图 195×39 共 5 帧；26px 窗口缩放后绿脸偏移 -109.3px */
.smiley img {
  height: 26px;
  margin-inline-start: -109.3px;
  max-width: none;
}

/* ── 参考图叠加 ── */
.reference-overlay {
  inset: 0;
  pointer-events: none;
  position: absolute;
}
</style>
