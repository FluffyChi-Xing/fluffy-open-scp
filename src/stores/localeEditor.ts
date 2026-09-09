import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import { isTauri, tauriApi } from "@/api";
import type {
  LocaleItem,
  LocaleTableSummary,
  LocaleTgi,
} from "@/api/tauri";
import { useToast } from "@/composables/useToast";

/**
 * Locale 文本编辑器（WP2）的全局状态：选中的 package / 表、编辑中的条目、
 * 脏标记。放在 Pinia 里使编辑进度在页面切换后保留；导出 overlay 后清脏。
 */
export const useLocaleEditorStore = defineStore("localeEditor", () => {
  const toast = useToast();

  const packageId = shallowRef<number | null>(null);
  const tables = shallowRef<LocaleTableSummary[]>([]);
  const loadingTables = shallowRef(false);
  const activeTgi = shallowRef<LocaleTgi | null>(null);
  // 深响应式：行内 v-model 直接改 item.text 需要触发依赖更新
  const items = ref<LocaleItem[]>([]);
  /** 原始条目快照（按 key），用于脏比较与还原。 */
  const baseline = shallowRef<Map<string, string>>(new Map());
  const loadingItems = shallowRef(false);
  const saving = shallowRef(false);
  const filter = shallowRef("");

  const dirtyCount = computed(
    () =>
      items.value.filter(
        (item) => baseline.value.get(item.key) !== item.text,
      ).length,
  );
  const hasEdits = computed(() => dirtyCount.value > 0);
  const filteredItems = computed(() => {
    const needle = filter.value.trim().toLowerCase();
    if (!needle) return items.value;
    return items.value.filter(
      (item) =>
        item.key.toLowerCase().includes(needle) ||
        item.text.toLowerCase().includes(needle),
    );
  });
  const activeTable = computed(
    () =>
      tables.value.find(
        (table) =>
          table.tgi.typeId === activeTgi.value?.typeId &&
          table.tgi.group === activeTgi.value?.group &&
          table.tgi.instance === activeTgi.value?.instance,
      ) ?? null,
  );

  async function loadTables(id: number) {
    if (!isTauri()) return;
    packageId.value = id;
    loadingTables.value = true;
    try {
      tables.value = await tauriApi.packages.locale.tables(id);
      activeTgi.value = null;
      items.value = [];
      baseline.value = new Map();
    } catch (cause) {
      tables.value = [];
      toast.error("Locale 表读取失败");
      console.error(cause);
    } finally {
      loadingTables.value = false;
    }
  }

  async function openTable(table: LocaleTableSummary) {
    if (packageId.value === null) return;
    activeTgi.value = table.tgi;
    loadingItems.value = true;
    filter.value = "";
    try {
      const response = await tauriApi.packages.locale.items(
        packageId.value,
        table.tgi,
      );
      items.value = response.items;
      baseline.value = new Map(response.items.map((item) => [item.key, item.text]));
    } catch (cause) {
      items.value = [];
      toast.error("Locale 条目读取失败");
      console.error(cause);
    } finally {
      loadingItems.value = false;
    }
  }

  function isDirty(item: LocaleItem) {
    return baseline.value.get(item.key) !== item.text;
  }

  function revert(item: LocaleItem) {
    const original = baseline.value.get(item.key);
    if (original !== undefined) item.text = original;
  }

  function revertAll() {
    for (const item of items.value) {
      const original = baseline.value.get(item.key);
      if (original !== undefined) item.text = original;
    }
  }

  async function exportOverlay() {
    if (packageId.value === null || !activeTgi.value || !hasEdits.value) return;
    saving.value = true;
    try {
      const defaultName = `openscp-locale-overlay.package`;
      const target = await tauriApi.packages.saveFile(defaultName, "package");
      if (!target) return;
      const result = await tauriApi.packages.locale.writeOverlay(
        [{ tgi: activeTgi.value, items: items.value }],
        target,
      );
      // 导出成功后把当前文本作为新基线（内容已落盘）
      baseline.value = new Map(items.value.map((item) => [item.key, item.text]));
      toast.success(
        `Overlay 已导出：${result.entryCount} 个资源，${result.bytesWritten} 字节`,
      );
    } catch (cause) {
      toast.error("Overlay 导出失败");
      console.error(cause);
    } finally {
      saving.value = false;
    }
  }

  return {
    packageId,
    tables,
    loadingTables,
    activeTgi,
    activeTable,
    items,
    loadingItems,
    saving,
    filter,
    filteredItems,
    dirtyCount,
    hasEdits,
    loadTables,
    openTable,
    isDirty,
    revert,
    revertAll,
    exportOverlay,
  };
});
