import { computed, shallowRef } from "vue";
import { defineStore } from "pinia";
import { isTauri, tauriApi } from "@/api";
import type { OverrideConflict, OverrideScanResponse } from "@/api/tauri";

/**
 * TGI 复写检测的全局状态：扫描 roots、结果、筛选与展开行。
 * 放在 Pinia 里使扫描结果在页面切换后保留（关闭 App 才丢弃）；
 * 游戏目录默认取设置页持久化的 gameDataPath。
 */
export const useOverrideScanStore = defineStore("overrideScan", () => {
  const gameDir = shallowRef("");
  const extraRoots = shallowRef("");
  const scanning = shallowRef(false);
  const error = shallowRef("");
  const result = shallowRef<OverrideScanResponse | null>(null);
  const filter = shallowRef("");
  const expanded = shallowRef<Set<number>>(new Set());
  let initialized = false;

  const roots = computed(() => {
    const extras = extraRoots.value
      .split(";")
      .map((item) => item.trim())
      .filter(Boolean);
    return [gameDir.value, ...extras].filter(Boolean);
  });

  /** 首次使用时从持久化设置读取游戏目录（此后不覆盖用户输入）。 */
  async function initFromSettings() {
    if (initialized || !isTauri()) return;
    initialized = true;
    if (gameDir.value) return;
    try {
      const settings = await tauriApi.settings.get();
      gameDir.value = settings.gameDataPath ?? "";
    } catch {
      // 读取失败时留空，由用户手动填写
    }
  }

  async function runScan() {
    if (!roots.value.length || scanning.value) return;
    scanning.value = true;
    error.value = "";
    try {
      result.value = await tauriApi.packages.overrideScan(roots.value);
      expanded.value = new Set();
    } catch (cause) {
      error.value =
        cause && typeof cause === "object" && "message" in cause
          ? String(cause.message)
          : String(cause);
    } finally {
      scanning.value = false;
    }
  }

  function toggleExpanded(index: number) {
    const next = new Set(expanded.value);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    expanded.value = next;
  }

  const conflicts = computed(() => {
    const all = result.value?.conflicts ?? [];
    const needle = filter.value.trim().toLowerCase();
    if (!needle) return all;
    return all.filter(
      (conflict: OverrideConflict) =>
        conflict.ext.toLowerCase().includes(needle) ||
        tgiText(conflict).toLowerCase().includes(needle) ||
        conflict.chain.some((item) => item.name.toLowerCase().includes(needle)),
    );
  });

  function tgiText(conflict: OverrideConflict) {
    return `${hex(conflict.typeId)}:${hex(conflict.groupId)}:${hex(conflict.instanceId)}`;
  }
  function hex(value: number) {
    return (value >>> 0).toString(16).toUpperCase().padStart(8, "0");
  }

  return {
    gameDir,
    extraRoots,
    scanning,
    error,
    result,
    filter,
    expanded,
    roots,
    conflicts,
    initFromSettings,
    runScan,
    toggleExpanded,
    tgiText,
  };
});
