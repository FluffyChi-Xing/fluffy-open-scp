import { createPinia, setActivePinia } from "pinia";
import { expect, it, vi } from "vitest";
import { useGamePackagesStore } from "./gamePackages";

const source = vi.hoisted(() => ({
  closePackage: vi.fn(),
}));
vi.mock("@/api/data-source", () => ({
  createDataSource: () => source,
  localDemoConfig: () => null,
}));

it("removes a tab on the first click even when its backend handle is already closed", async () => {
  setActivePinia(createPinia());
  const store = useGamePackagesStore();
  const page = { items: [], offset: 0, total: 0, limit: 100, typeCounts: [] };
  store.opened = [1, 2].map((packageId) => ({
    package: {
      packageId, path: `${packageId}.package`, kind: "Dbpf", size: 0,
      majorVersion: 1, minorVersion: 0, entryCount: 0,
    },
    resources: page,
  }));
  store.choosePackage(1);
  let rejectClose!: (error: { code: string }) => void;
  source.closePackage.mockImplementationOnce(() => new Promise((_resolve, reject) => {
    rejectClose = reject;
  }));

  const closing = store.closePackage(1);
  expect(store.opened.map((item) => item.package.packageId)).toEqual([2]);
  expect(store.activePackageId).toBe(2);
  await store.closePackage(1);
  rejectClose({ code: "not_found" });
  await closing;
  expect(source.closePackage).toHaveBeenCalledTimes(1);
  expect(store.opened.map((item) => item.package.packageId)).toEqual([2]);
});
