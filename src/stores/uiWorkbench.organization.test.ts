import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as assets from "@/lib/game-assets";
import * as runtime from "@/api";
import type { ReplicaData } from "@/lib/game-ui/replica/types";
import { useUiWorkbenchStore } from "./uiWorkbench";

export function organizationFixture(): ReplicaData {
  const tool = { instance: "AAAAAAAA", label: "Fire station", preview: null, marquee: null, locked: false };
  return {
    layout: { children: [] }, assets: {}, locale: {},
    model: { categories: ["11111111", "22222222", "33333333"].map(id => ({ id, label: id, icon: null, iconHash: null })),
      row: { pitch: 40, size: 40, centerX: 800, offsetY: 0 } },
    tools: { "11111111": [tool], "22222222": [{ ...tool }], "33333333": [] },
    palette: { byCategory: {}, row: { pitch: 100, startX: 200, slotW: 100, slotH: 50, topInBar: 4 } },
  } as unknown as ReplicaData;
}

describe("native menu organization", () => {
  beforeEach(() => setActivePinia(createPinia()));
  afterEach(() => vi.restoreAllMocks());
  function setup() {
    const store = useUiWorkbenchStore();
    store.hydrate(organizationFixture());
    vi.spyOn(runtime, "isTauri").mockReturnValue(true);
    const read = vi.spyOn(assets, "gamePropertyRead").mockResolvedValue({
      typeId: 0xb1b104, group: 0x40e1c900, instance: 0xaaaaaaaa,
      keys: [{ hash: 0x0db9fc63, array: true, values: [0x11111111, 0x22222222, 0x44444444]
        .map(instance => ({ kind: "key", key: [0, 0, instance] as [number, number, number] })) }],
    });
    const copy = vi.spyOn(assets, "gamePropertyCopy").mockResolvedValue({ dataBase64: "AAAAAA==", keyCount: 0, sourceGroup: 0x40e1c900 });
    return { store, read, copy };
  }
  it("moves without a new identity, preserves other native memberships and source group across views", async () => {
    const { store, copy } = setup();
    store.updateItem("11111111", "AAAAAAAA", { label: "Edited" });
    const moved = store.moveEntry("11111111", "AAAAAAAA", "33333333")!;
    expect(moved.tool.id).toBe("AAAAAAAA");
    expect(moved.tool.label).toBe("Edited");
    expect(store.menuEntries("11111111")).toEqual([]);
    expect(store.menuEntries("33333333")).toHaveLength(1);
    expect(store.menuEntries("22222222")).toHaveLength(1);
    store.setViewMode("development");
    const result = await store.buildGameExport();
    expect(result.warnings).toEqual([]);
    expect(copy).toHaveBeenCalledTimes(1);
    expect(copy.mock.calls[0][0]).toMatchObject({ sourceInstance: 0xaaaaaaaa, newInstance: 0xaaaaaaaa,
      sourceGroup: 0x40e1c900, uiCategories: [0x22222222, 0x44444444, 0x33333333] });
    const view = new DataView(result.bytes.buffer);
    expect(view.getUint32(104 + 4, true)).toBe(0x40e1c900);
  });
  it("repeated moves and returning to the source never duplicate an item", async () => {
    const { store, copy } = setup();
    store.moveEntry("11111111", "AAAAAAAA", "33333333");
    store.moveEntry("33333333", "AAAAAAAA", "11111111");
    expect(store.menuEntries("11111111")).toHaveLength(1);
    expect(store.menuEntries("33333333")).toEqual([]);
    await store.buildGameExport();
    expect(copy.mock.calls[0][0].uiCategories).toEqual([0x22222222, 0x44444444, 0x11111111]);
  });
  it("creates exportable empty groups and moves selected items into them", async () => {
    const { store, copy } = setup();
    const compile = vi.spyOn(assets, "gameMenuGroupsCompile").mockResolvedValue([
      { typeId: 0xb1b104, group: 0xf1000001, instance: 0xf2000001, dataBase64: "AAAAAA==" },
    ]);
    const id = store.createMenuGroup("33333333", "Emergency services")!;
    expect(id).toMatch(/^[0-9A-F]{8}$/);
    expect(store.createMenuGroup("33333333", "Emergency services")).not.toBe(id);
    const empty = await store.buildGameExport();
    expect(empty.entryCount).toBeGreaterThan(0);
    expect(copy).not.toHaveBeenCalled();
    store.moveEntry("11111111", "AAAAAAAA", id);
    store.enterMenu(id);
    expect(store.activeMenu?.entries[0].tool.id).toBe("AAAAAAAA");
    expect(store.groupTabs("33333333").find(tab => tab.label === "Emergency services")?.instances).toEqual(["AAAAAAAA"]);
    await store.buildGameExport();
    expect(compile.mock.calls.at(-1)?.[0].children).toHaveLength(2);
    expect(copy.mock.calls[0][0].uiCategories).toContain(Number.parseInt(id, 16));
    const snapshot = JSON.parse(store.exportOverlay());
    expect(snapshot.menuGroups).toHaveLength(2);
    expect(snapshot.moves.AAAAAAAA).toHaveLength(1);
    store.resetAll();
    expect(store.menuGroups).toEqual([]);
    expect(store.moves).toEqual({});
    expect(store.activeMenu).toBeNull();
  });
  it("keeps existing root items in an explicit ungrouped tab when a group is created", () => {
    const { store } = setup();
    const id = store.createMenuGroup("11111111", "Emergency services")!;
    expect(store.menuEntries("11111111")).toHaveLength(1);
    expect(store.groupTabs("11111111")).toEqual([
      { label: "未分组", instances: ["AAAAAAAA"] },
      { label: "Emergency services", instances: [] },
    ]);
    expect(store.menuEntries(id)).toEqual([]);
  });
  it("refuses a lossy fallback when membership reads or full resource copies fail", async () => {
    const { store, read, copy } = setup();
    store.moveEntry("11111111", "AAAAAAAA", "33333333");
    read.mockResolvedValueOnce(null);
    await expect(store.buildGameExport()).rejects.toThrow();
    read.mockResolvedValueOnce({ typeId: 0xb1b104, group: 0x40e1c900, instance: 0xaaaaaaaa,
      keys: [{ hash: 0x0db9fc63, array: true, values: [{ kind: "key", key: [0, 0, 0x55555555] }] }] });
    await expect(store.buildGameExport()).rejects.toThrow();
    copy.mockRejectedValueOnce(new Error("unavailable"));
    await expect(store.buildGameExport()).rejects.toThrow();
    vi.spyOn(runtime, "isTauri").mockReturnValue(false);
    await expect(store.buildGameExport()).rejects.toThrow();
  });
  it("moving a copied entry retains its already allocated export identity", async () => {
    const { store, copy } = setup();
    const duplicate = store.duplicateToMenu("11111111", "AAAAAAAA", "33333333")!;
    await store.buildGameExport();
    const instance = copy.mock.calls[0][0].newInstance;
    copy.mockClear();
    store.moveEntry("33333333", duplicate.tool.id, "11111111");
    await store.buildGameExport();
    expect(copy.mock.calls[0][0]).toMatchObject({ newInstance: instance, sourceInstance: 0xaaaaaaaa, uiCategories: [0x11111111] });
    expect(store.menuEntries("33333333")).toEqual([]);
    expect(store.menuEntries("11111111")).toHaveLength(2);
  });
});
