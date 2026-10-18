<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import MenuTargetTree, { type CopyTargetNode } from "./MenuTargetTree.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import { Switch } from "@/components/ui/switch";
import type { ReplicaSubmenuItem } from "@/lib/game-ui/replica/types";
const { t } = useI18n();

/**
 * 菜单条目 Sheet 的分区表单体：基本 / 归属与复制 / 图像 / 文案 / 经济 /
 * 统计行 / 子菜单。所有编辑只作用于草稿（模型），应用时由宿主做精确 diff
 * 写入覆盖 —— 编辑器编辑的是副本，绝不改基础数据。
 */
const model = defineModel<{
  label: string;
  pos: number;
  icon: string;
  desc: string;
  cost: string;
  upkeep: string;
  unlock: string;
  locked: boolean;
  preview: string | null;
  marquee: string | null;
  stats: { label: string; value: string }[];
}>({ required: true });

defineProps<{
  memberships: { id: string; label: string }[];
  copyTargets: CopyTargetNode[];
  /** 建筑子菜单条目（costs.json 反查；无则不渲染该分区）。 */
  submenus?: ReplicaSubmenuItem[];
}>();

const emit = defineEmits<{
  (e: "crop", target: "preview" | "marquee"): void;
  (e: "copy", target: CopyTargetNode): void;
  (e: "open-submenu", item: ReplicaSubmenuItem): void;
}>();

/* ── 复制到菜单 ── */
const copyTarget = ref<CopyTargetNode | null>(null);
const copyReady = computed(() => copyTarget.value !== null);
function onCopy(): void {
  if (!copyTarget.value) return;
  emit("copy", copyTarget.value);
  copyTarget.value = null;
}

/* ── 统计行 ── */
function addStat(): void {
  model.value.stats = [...model.value.stats, { label: "", value: "" }];
}
function removeStat(index: number): void {
  model.value.stats = model.value.stats.filter((_, i) => i !== index);
}
</script>

<template>
  <div class="sheet-form">
    <!-- 基本 -->
    <section class="form-section">
      <h3 class="section-title">{{ t('studio.workbench.sectionBasics') }}</h3>
      <label class="field">
        <span>{{ t('studio.workbench.fieldLabel') }}</span>
        <input v-model="model.label" type="text" />
      </label>
      <div class="field-row">
        <label class="field">
          <span>{{ t('studio.workbench.fieldPos') }}</span>
          <input v-model.number="model.pos" type="number" />
        </label>
        <label class="field">
          <span>{{ t('studio.workbench.fieldIcon') }}</span>
          <input v-model="model.icon" type="text" placeholder="Box" />
        </label>
      </div>
      <p class="field-hint">{{ t('studio.workbench.iconHint') }}</p>
    </section>

    <!-- 归属与复制（嵌套的子菜单条目同样可复制） -->
    <section class="form-section">
      <h3 class="section-title">{{ t('studio.workbench.sectionMembership') }}</h3>
      <p class="member-chips">
        <span v-for="menu in memberships" :key="menu.id" class="chip">{{ menu.label }}</span>
        <span v-if="memberships.length === 0" class="chip none">{{ t('studio.workbench.membershipNone') }}</span>
      </p>
      <div class="field">
        <span>{{ t('studio.workbench.copyTo') }}</span>
        <div class="copy-row">
          <MenuTargetTree
            v-model="copyTarget"
            :nodes="copyTargets"
            :placeholder="t('studio.workbench.copyTargetPlaceholder')"
          />
          <button type="button" class="copy-btn" :disabled="!copyReady" @click="onCopy">
            {{ t('studio.workbench.copyAction') }}
          </button>
        </div>
        <p class="field-hint">{{ t('studio.workbench.copyHint') }}</p>
      </div>
    </section>

    <!-- 图像 -->
    <section class="form-section">
      <h3 class="section-title">{{ t('studio.workbench.sectionImages') }}</h3>
      <div class="field-row">
        <div class="field">
          <span>{{ t('studio.workbench.slotIcon') }}</span>
          <div class="img-wrap square">
            <button type="button" class="img-btn" @click="emit('crop', 'preview')">
              <img v-if="model.preview" :src="model.preview" alt="" />
              <span v-else class="img-btn-hint">{{ t('studio.workbench.uploadSlot') }}</span>
            </button>
            <button
              v-if="model.preview"
              type="button"
              class="img-remove"
              :title="t('studio.workbench.removeImage')"
              :aria-label="t('studio.workbench.removeImage')"
              @click="model.preview = null"
            >✕</button>
          </div>
        </div>
        <div class="field">
          <span>{{ t('studio.workbench.tipImage') }}</span>
          <div class="img-wrap">
            <button type="button" class="img-btn" @click="emit('crop', 'marquee')">
              <img v-if="model.marquee" :src="model.marquee" alt="" />
              <span v-else class="img-btn-hint">{{ t('studio.workbench.uploadTip') }}</span>
            </button>
            <button
              v-if="model.marquee"
              type="button"
              class="img-remove"
              :title="t('studio.workbench.removeImage')"
              :aria-label="t('studio.workbench.removeImage')"
              @click="model.marquee = null"
            >✕</button>
          </div>
        </div>
      </div>
      <p class="field-hint">{{ t('studio.workbench.imageHint') }}</p>
    </section>

    <!-- 文案 -->
    <section class="form-section">
      <h3 class="section-title">{{ t('studio.workbench.sectionText') }}</h3>
      <label class="field">
        <span>{{ t('studio.workbench.tipDescription') }}</span>
        <textarea v-model="model.desc" rows="3" />
      </label>
      <label class="field">
        <span>{{ t('studio.workbench.unlock') }}</span>
        <input v-model="model.unlock" type="text" />
      </label>
    </section>

    <!-- 经济 -->
    <section class="form-section">
      <h3 class="section-title">{{ t('studio.workbench.sectionEconomy') }}</h3>
      <div class="field-row">
        <label class="field">
          <span>{{ t('studio.workbench.cost') }}</span>
          <input v-model="model.cost" type="text" :placeholder="t('studio.workbench.costPlaceholder')" />
        </label>
        <label class="field">
          <span>{{ t('studio.workbench.upkeep') }}</span>
          <input v-model="model.upkeep" type="text" :placeholder="t('studio.workbench.upkeepPlaceholder')" />
        </label>
      </div>
      <label class="field-check">
        <Switch v-model="model.locked" :aria-label="t('studio.workbench.locked')" />
        <span>{{ t('studio.workbench.locked') }}</span>
      </label>
      <p class="field-hint">{{ t('studio.workbench.financeHint') }}</p>
    </section>

    <!-- 统计行 -->
    <section class="form-section">
      <h3 class="section-title">
        {{ t('studio.workbench.sectionStats') }}
        <button type="button" class="mini-btn" @click="addStat">
          <FIcon name="Plus" :size="12" aria-label="" /> {{ t('studio.workbench.statAdd') }}
        </button>
      </h3>
      <p class="field-hint">{{ t('studio.workbench.statsHint') }}</p>
      <div v-for="(stat, index) in model.stats" :key="index" class="stat-row">
        <input v-model="stat.label" type="text" :placeholder="t('studio.workbench.statLabelPlaceholder')" />
        <input v-model="stat.value" type="text" :placeholder="t('studio.workbench.statValuePlaceholder')" />
        <button
          type="button"
          class="img-remove static"
          :title="t('studio.workbench.removeImage')"
          :aria-label="t('studio.workbench.statRemove')"
          @click="removeStat(index)"
        >✕</button>
      </div>
    </section>

    <!-- 子菜单 -->
    <section v-if="submenus && submenus.length > 0" class="form-section">
      <h3 class="section-title">{{ t('studio.workbench.sectionSubmenu') }}</h3>
      <p class="field-hint">{{ t('studio.workbench.submenuHint') }}</p>
      <button
        v-for="item in submenus"
        :key="item.instance"
        type="button"
        class="submenu-item"
        @click="emit('open-submenu', item)"
      >
        <span class="submenu-thumb">
          <img v-if="item.preview" :src="item.preview" alt="" />
          <FIcon v-else name="Box" :size="18" aria-label="" />
        </span>
        <span class="submenu-body">
          <span class="submenu-name">{{ item.name }}</span>
          <span class="submenu-meta">
            {{ t('studio.workbench.costShort') }} {{ item.cost ?? "—" }}
            <template v-if="item.maintenance != null">
              · {{ t('studio.workbench.upkeepShort') }} {{ item.maintenance }}
            </template>
          </span>
        </span>
        <FIcon class="submenu-chev" name="ChevronRight" :size="14" aria-label="" />
      </button>
    </section>
  </div>
</template>

<style scoped>
.sheet-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.form-section {
  border-top: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-top: 12px;
}
.form-section:first-child {
  border-top: 0;
  padding-top: 0;
}
.section-title {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  font-weight: 700;
  gap: 8px;
  justify-content: space-between;
  letter-spacing: 0.06em;
  margin: 0;
  text-transform: uppercase;
}
.field {
  display: flex;
  flex-direction: column;
  font-size: 12px;
  gap: 4px;
  min-width: 0;
}
.field > span {
  color: var(--muted-foreground);
}
.field-row {
  display: grid;
  gap: 12px;
  grid-template-columns: 1fr 1fr;
}
.field input,
.field textarea {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 13px;
  height: 32px;
  min-width: 0;
  padding: 5px 9px;
  width: 100%;
}
.field textarea {
  height: auto;
  resize: vertical;
}
.field-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 0;
}
.field-check {
  align-items: center;
  display: flex;
  flex-direction: row;
  gap: 6px;
}
.field-check span {
  color: var(--foreground);
  font-size: 12px;
}
.member-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin: 0;
}
.chip {
  background: var(--surface-hover);
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--foreground);
  font-size: 11px;
  padding: 2px 9px;
}
.chip.none {
  color: var(--muted-foreground);
}
.copy-row {
  display: grid;
  gap: 8px;
  grid-template-columns: 1fr auto;
}
.copy-row :deep(.f-select) {
  height: 32px;
  min-height: 32px;
  font-size: 13px;
}
.copy-row :deep(.f-select-anchor) {
  display: flex;
}
.copy-row :deep(.f-select-anchor .f-dropdown-anchor) {
  width: 100%;
}
.copy-btn {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  font-size: 12px;
  height: 32px;
  padding: 0 12px;
  white-space: nowrap;
}
.copy-btn:hover:not(:disabled) {
  border-color: var(--primary);
  color: var(--primary);
}
.copy-btn:disabled {
  cursor: default;
  opacity: 0.45;
}
.img-wrap {
  position: relative;
}
.img-wrap.square .img-btn {
  height: 84px;
}
.img-btn {
  align-items: center;
  background: var(--surface);
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  height: 96px;
  justify-content: center;
  overflow: hidden;
  width: 100%;
}
.img-btn:hover {
  border-color: var(--primary, #0b78fe);
}
.img-btn img {
  height: 100%;
  object-fit: cover;
  width: 100%;
}
.img-wrap.square .img-btn img {
  object-fit: contain;
}
.img-btn-hint {
  font-size: 11px;
  padding: 0 6px;
  text-align: center;
}
.img-remove {
  align-items: center;
  background: #d3242a;
  border: 1px solid #fff;
  border-radius: 50%;
  color: #fff;
  cursor: pointer;
  display: inline-flex;
  font-size: 11px;
  height: 18px;
  justify-content: center;
  line-height: 1;
  position: absolute;
  right: 6px;
  top: 6px;
  width: 18px;
}
.img-remove.static {
  position: static;
  flex: none;
}
.mini-btn {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font-size: 11px;
  gap: 4px;
  padding: 3px 8px;
}
.mini-btn:hover {
  border-color: var(--primary);
  color: var(--primary);
}
.stat-row {
  display: grid;
  gap: 6px;
  grid-template-columns: 1fr 1fr auto;
}
/* 与字段输入同规格（裸 input 会继承 16px，视觉过大） */
.stat-row input {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 13px;
  height: 32px;
  min-width: 0;
  padding: 5px 9px;
  width: 100%;
}
.submenu-item {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  cursor: pointer;
  display: flex;
  gap: 10px;
  padding: 8px 10px;
  text-align: start;
  transition: background-color 120ms ease, border-color 120ms ease;
  width: 100%;
}
.submenu-item:hover {
  background: var(--surface-hover);
  border-color: var(--border-strong);
}
.submenu-thumb {
  align-items: center;
  background: var(--surface-hover);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: inline-flex;
  flex: none;
  height: 40px;
  justify-content: center;
  overflow: hidden;
  width: 40px;
}
.submenu-thumb img {
  height: 100%;
  object-fit: contain;
  width: 100%;
}
.submenu-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.submenu-name {
  color: var(--foreground);
  font-size: 12.5px;
  font-weight: 600;
}
.submenu-meta {
  color: var(--muted-foreground);
  font-size: 10.5px;
}
.submenu-chev {
  color: var(--muted-foreground);
  flex: none;
  margin-inline-start: auto;
}
</style>
