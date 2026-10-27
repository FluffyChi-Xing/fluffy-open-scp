import { computed, ref } from "vue";
import { command } from "@/api/tauri";
import { isTauri } from "@/api";
import { i18n } from "@/locales";
import type { DbpfEntry } from "./dbpf-writer";

export interface CountLimits {
  individual: number | null;
  shared: number | null;
  sharedKey: string | null;
}
export interface CountLimitDraft {
  source: string;
  individual: number | string | null;
  shared: number | string | null;
}
export const validCountLimit = (value: unknown): value is number =>
  typeof value === "number" && Number.isInteger(value) && value >= 1 && value <= 16_777_216;

/** Store-owned edits: copies of a menu still target the same simulation unit;
 * shared budgets have one value across all menus and views. */
export function useCountLimits() {
  const metadata = ref<Record<string, CountLimits>>({});
  const individualEdits = ref<Record<string, number>>({});
  const sharedEdits = ref<Record<string, { value: number; source: string }>>({});
  const editCount = computed(() => Object.keys(individualEdits.value).length + Object.keys(sharedEdits.value).length);
  const requests = new Map<string, Promise<CountLimits>>();

  async function load(source: string): Promise<CountLimits> {
    if (metadata.value[source]) return effective(source)!;
    let request = requests.get(source);
    if (!request) {
      request = (async () => {
        let result: CountLimits;
        if (isTauri()) {
          result = await command<CountLimits>("game_count_limits_read", { sourceInstance: Number.parseInt(source, 16) });
        } else {
          const response = await fetch("/game-ui/tools/count-limits.json");
          if (!response.ok) throw new Error("count metadata unavailable");
          const manifest = await response.json() as Record<string, CountLimits>;
          result = manifest[source] ?? { individual: null, shared: null, sharedKey: null };
        }
        metadata.value[source] = result;
        return result;
      })();
      requests.set(source, request);
    }
    try { await request; return effective(source)!; }
    finally { requests.delete(source); }
  }

  function effective(source: string): CountLimits | null {
    const base = metadata.value[source];
    if (!base) return null;
    return {
      ...base,
      individual: individualEdits.value[source] ?? base.individual,
      shared: (base.sharedKey ? sharedEdits.value[base.sharedKey]?.value : undefined) ?? base.shared,
    };
  }

  function apply(draft: CountLimitDraft): void {
    const base = metadata.value[draft.source];
    if (!base) throw new Error(i18n.global.t("studio.workbench.countLimitsLoadFailed"));
    for (const field of ["individual", "shared"] as const) {
      if (base[field] != null && !validCountLimit(draft[field])) {
        throw new Error(i18n.global.t("studio.workbench.countLimitsInvalid"));
      }
    }
    if (base.individual != null) {
      if (draft.individual === base.individual) delete individualEdits.value[draft.source];
      else individualEdits.value[draft.source] = draft.individual as number;
    }
    if (base.shared != null && base.sharedKey) {
      if (draft.shared === base.shared) delete sharedEdits.value[base.sharedKey];
      else sharedEdits.value[base.sharedKey] = { value: draft.shared as number, source: draft.source };
    }
  }

  function snapshot() { return { individual: individualEdits.value, shared: sharedEdits.value }; }
  function reset() { individualEdits.value = {}; sharedEdits.value = {}; }

  async function compile(base: DbpfEntry[]): Promise<DbpfEntry[]> {
    if (!editCount.value) return [];
    if (!isTauri()) throw new Error(i18n.global.t("studio.workbench.countLimitsBackendRequired"));
    const changes = new Map<string, { sourceInstance: number; individual?: number; shared?: number }>();
    for (const [source, individual] of Object.entries(individualEdits.value)) {
      changes.set(source, { sourceInstance: Number.parseInt(source, 16), individual });
    }
    for (const { value, source } of Object.values(sharedEdits.value)) {
      changes.set(source, { ...changes.get(source), sourceInstance: Number.parseInt(source, 16), shared: value });
    }
    const baseResources = base.filter((r) => r.type === 0x00b1b104).map((r) => {
      let binary = "";
      for (let start = 0; start < r.data.length; start += 8192) binary += String.fromCharCode(...r.data.subarray(start, start + 8192));
      return { typeId: r.type, group: r.group, instance: r.instance, dataBase64: btoa(binary) };
    });
    const resources = await command<{ typeId: number; group: number; instance: number; dataBase64: string }[]>(
      "game_count_limits_patch", { changes: [...changes.values()], baseResources },
    ).catch((error: unknown) => {
      console.error("[count-limits] resource compilation failed", error);
      throw new Error(i18n.global.t("studio.workbench.countLimitsWriteFailed"));
    });
    return resources.map((r) => ({ type: r.typeId, group: r.group, instance: r.instance, data: Uint8Array.from(atob(r.dataBase64), (c) => c.charCodeAt(0)) }));
  }
  return { editCount, load, effective, apply, snapshot, reset, compile };
}
