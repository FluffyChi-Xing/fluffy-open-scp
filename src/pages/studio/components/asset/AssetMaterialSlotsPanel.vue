<script setup lang="ts">
import { computed, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { command } from "@/api/tauri";
import type { MaterialSlotBinding } from "./useWorkbenchAsset";

/**
 * 右栏 "材质槽"：绑定管线契约的六槽文件（params.json + slot1..5.png，
 * 项目内已导入的相对路径），组装 LotMaterialTextures 后走原生 tint
 * 着色器——契约 GLB 本身不含贴图（导出即 NONE），真贴图观感由这套
 * 绑定驱动。全部解绑 = 回到 COLOR_0 调色板渲染。
 */
const props = defineProps<{
  project: string;
  binding: MaterialSlotBinding;
  busy: boolean;
  disabled: boolean;
}>();
const emit = defineEmits<{
  bind: [binding: MaterialSlotBinding];
}>();

const { t } = useI18n();
const importing = ref("");

const SLOTS = [
  { key: "params", icon: "Braces", label: "studio.assetWorkbench.slotParams", accept: ["json"], hint: "studio.assetWorkbench.slotParamsHint" },
  { key: "slot1", icon: "Image", label: "studio.assetWorkbench.slot1", accept: ["png"], hint: "studio.assetWorkbench.slot1Hint" },
  { key: "slot2", icon: "Image", label: "studio.assetWorkbench.slot2", accept: ["png"], hint: "studio.assetWorkbench.slot2Hint" },
  { key: "slot3", icon: "Image", label: "studio.assetWorkbench.slot3", accept: ["png"], hint: "studio.assetWorkbench.slot3Hint" },
  { key: "slot4", icon: "Image", label: "studio.assetWorkbench.slot4", accept: ["png"], hint: "studio.assetWorkbench.slot4Hint" },
  { key: "slot5", icon: "Image", label: "studio.assetWorkbench.slot5", accept: ["png"], hint: "studio.assetWorkbench.slot5Hint" },
] as const;
type SlotKey = (typeof SLOTS)[number]["key"];

const boundCount = computed(
  () => SLOTS.filter(({ key }) => props.binding[key]).length,
);

async function pickSlot(key: SlotKey, accept: readonly string[]) {
  const path = await open({
    multiple: false,
    filters: [
      {
        name: t("studio.assetWorkbench.importFilterName"),
        extensions: [...accept],
      },
    ],
  });
  if (typeof path !== "string") return;
  importing.value = key;
  try {
    // 复制进项目 assets/textures（hash 命名），绑定存项目相对路径
    const value = await command<{ asset: string }>("code_flow", {
      request: { project: props.project, action: "import-asset", payload: { path } },
    });
    emit("bind", { ...props.binding, [key]: value.asset });
  } finally {
    importing.value = "";
  }
}

function unbindSlot(key: SlotKey) {
  const next = { ...props.binding };
  delete next[key];
  emit("bind", next);
}

function fileName(path?: string) {
  return path ? (path.split(/[\\/]/).pop() ?? path) : "";
}
</script>

<template>
  <div class="slot-panel" :aria-label="t('studio.assetWorkbench.materialSlotsTab')">
    <p class="slot-hint">{{ t("studio.assetWorkbench.slotPanelHint") }}</p>
    <div
      v-for="slot in SLOTS"
      :key="slot.key"
      class="slot-row"
    >
      <FIcon :name="slot.icon" :size="14" aria-label="" />
      <div class="slot-meta">
        <strong>{{ t(slot.label) }}</strong>
        <small :title="binding[slot.key]">
          {{ fileName(binding[slot.key]) || t("studio.assetWorkbench.slotUnbound") }}
        </small>
      </div>
      <span
        v-if="importing === slot.key"
        class="slot-busy"
      >
        <FSpinner size="sm" :label="t('common.loading')" />
      </span>
      <template v-else>
        <button
          type="button"
          class="slot-btn"
          :disabled="disabled"
          :title="t(slot.hint)"
          @click="pickSlot(slot.key, slot.accept)"
        >
          <FIcon name="FilePlus" :size="12" aria-label="" />
          {{ t("studio.assetWorkbench.slotBind") }}
        </button>
        <button
          v-if="binding[slot.key]"
          type="button"
          class="slot-btn danger"
          :disabled="disabled"
          :aria-label="t('studio.assetWorkbench.slotUnbind')"
          @click="unbindSlot(slot.key)"
        >
          <FIcon name="X" :size="12" aria-label="" />
        </button>
      </template>
    </div>
    <p class="slot-state">
      {{
        boundCount
          ? t("studio.assetWorkbench.slotBoundCount", { count: boundCount })
          : t("studio.assetWorkbench.slotNoneBound")
      }}
    </p>
  </div>
</template>

<style scoped>
.slot-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 100%;
  min-height: 0;
  overflow: auto;
  padding: 10px;
}
.slot-hint {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.55;
  margin: 0;
}
.slot-row {
  align-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: flex;
  gap: 8px;
  padding: 7px 8px;
}
.slot-meta {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.slot-meta strong {
  color: var(--foreground);
  font-size: 11.5px;
}
.slot-meta small {
  color: var(--muted-foreground);
  font-size: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.slot-busy {
  flex: none;
}
.slot-btn {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  flex: none;
  font: inherit;
  font-size: 10.5px;
  gap: 4px;
  min-height: 24px;
  padding: 0 8px;
  white-space: nowrap;
}
.slot-btn:hover:not(:disabled) {
  border-color: var(--primary);
  color: var(--primary);
}
.slot-btn:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.slot-btn.danger:hover:not(:disabled) {
  border-color: var(--danger);
  color: var(--danger);
}
.slot-state {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
  text-align: center;
}
</style>
