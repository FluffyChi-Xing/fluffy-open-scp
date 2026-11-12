import { createPinia, setActivePinia } from "pinia";
import { expect, it, vi } from "vitest";
import type { ResourcePage } from "@/api/tauri";
import { useGamePackagesStore } from "./gamePackages";

const source = vi.hoisted(() => ({
  listResources: vi.fn(),
  closePackage: vi.fn(),
}));
vi.mock("@/api/data-source", () => ({
  createDataSource: () => source,
  localDemoConfig: () => null,
}));

it("keeps the first unfiltered page when an older search finishes after clearing", async () => {
  setActivePinia(createPinia());
  const store = useGamePackagesStore();
  const unfiltered: ResourcePage = {
    items: [], offset: 0, total: 200, limit: 100, typeCounts: [],
  };
  store.opened = [{
    package: {
      packageId: 1, path: "sample.package", kind: "Dbpf", size: 0,
      majorVersion: 1, minorVersion: 0, entryCount: unfiltered.total,
    },
    resources: unfiltered,
  }];
  store.activePackageId = 1;
  const filtered = { ...unfiltered, total: 1 };
  let finishSearch!: (page: ResourcePage) => void;
  source.listResources
    .mockReturnValueOnce(new Promise<ResourcePage>((resolve) => { finishSearch = resolve; }))
    .mockResolvedValueOnce(unfiltered);

  const searching = store.applyFilter("tower");
  await store.applyFilter("");
  finishSearch(filtered);
  await searching;

  expect(source.listResources).toHaveBeenLastCalledWith(1, 0, 100, undefined, undefined);
  expect(store.filter).toBe("");
  expect(store.activePage).toEqual(unfiltered);
  expect(store.loadingPackage).toBe(false);
});

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
