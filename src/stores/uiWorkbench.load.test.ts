/** @summary A slower preview load cannot overwrite the user's newer language or generation. */
import { createPinia, setActivePinia } from "pinia";
import { expect, it, vi } from "vitest";
import type { ReplicaData } from "@/lib/game-ui/replica/types";
const loader = vi.hoisted(() => ({ loadReplicaData: vi.fn() }));
vi.mock("@/lib/game-ui/replica/loader", () => loader);
import { useUiWorkbenchStore } from "./uiWorkbench";

/** @summary Minimal valid data without optional legacy park backfill. */
function fixture(generation: string): ReplicaData {
  return { previewGeneration: generation, layout: {}, assets: {}, locale: {}, model: { categories: [], row: { pitch: 1, size: 1, centerX: 1, offsetY: 0 } }, tools: {}, palette: { byCategory: {}, row: { pitch: 1, startX: 0, slotW: 1, slotH: 1, topInBar: 0 } } };
}

it("keeps the newest result when earlier work completes last", async () => {
  setActivePinia(createPinia());
  let completeOld!: (data: ReplicaData) => void;
  loader.loadReplicaData.mockReturnValueOnce(new Promise<ReplicaData>((resolve) => { completeOld = resolve; }));
  loader.loadReplicaData.mockResolvedValueOnce(fixture("new"));
  const store = useUiWorkbenchStore();
  const old = store.load("en-US");
  await store.load("pl-PL");
  completeOld(fixture("old"));
  await old;
  expect(store.replica?.previewGeneration).toBe("new");
  expect(store.gameLocale).toBe("pl-PL");
  expect(store.loading).toBe(false);
});

it("clears an older preview when replacement fails", async () => {
  setActivePinia(createPinia());
  const store = useUiWorkbenchStore();
  store.hydrate(fixture("old"));
  loader.loadReplicaData.mockRejectedValueOnce(new Error("changed generation"));
  const loading = store.load("pl-PL");
  expect(store.replica).toBeNull();
  await loading;
  expect(store.replica).toBeNull();
  expect(store.loadError).toBe("changed generation");
});
