import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({
  tauri: true,
  targets: vi.fn(),
  changesets: vi.fn(),
  detail: vi.fn(),
  captureBaseline: vi.fn(),
  rollback: vi.fn(),
  remove: vi.fn(),
}));

vi.mock("@/api", () => ({
  isTauri: () => state.tauri,
  tauriApi: {
    versions: {
      targets: state.targets,
      changesets: state.changesets,
      detail: state.detail,
      captureBaseline: state.captureBaseline,
      rollback: state.rollback,
      remove: state.remove,
    },
  },
}));

const { useVersionConsoleStore } = await import("./versionConsole");

function summary(revision: number) {
  return {
    id: revision,
    targetPath: "C:/a.package",
    writer: "locale_overlay",
    revision,
    resourceCount: 1,
    bytesBefore: 0,
    bytesAfter: 10,
    createdAt: 1,
  };
}

describe("useVersionConsoleStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    state.tauri = true;
    state.targets.mockReset().mockResolvedValue([
      { targetPath: "C:/a.package", latestRevision: 2, changesetCount: 2, lastWrittenAt: 1 },
    ]);
    state.changesets.mockReset().mockResolvedValue([summary(2), summary(1)]);
    state.detail.mockReset().mockResolvedValue({ changeset: summary(2), items: [] });
    state.captureBaseline.mockReset().mockResolvedValue({ created: true, changeset: summary(1) });
  });

  it("loadTargets 填充已跟踪文件", async () => {
    const store = useVersionConsoleStore();
    await store.loadTargets();
    expect(store.targets).toHaveLength(1);
    expect(store.targets[0].latestRevision).toBe(2);
  });

  it("openTarget 切换目标并拉取该文件的版本线", async () => {
    const store = useVersionConsoleStore();
    await store.openTarget("C:/a.package");
    expect(store.activeTarget).toBe("C:/a.package");
    expect(state.changesets).toHaveBeenCalledWith("C:/a.package");
    expect(store.changesets.map((entry) => entry.revision)).toEqual([2, 1]);
  });

  it("captureBaseline 成功后刷新列表与时间线", async () => {
    const store = useVersionConsoleStore();
    await store.openTarget("C:/a.package");
    state.targets.mockClear();
    state.changesets.mockClear();

    await store.captureBaseline();

    expect(state.captureBaseline).toHaveBeenCalledWith("C:/a.package");
    expect(state.targets).toHaveBeenCalledTimes(1);
    expect(state.changesets).toHaveBeenCalledTimes(1);
  });

  it("非 Tauri 环境不调用后端", async () => {
    state.tauri = false;
    const store = useVersionConsoleStore();
    await store.loadTargets();
    await store.openTarget("C:/a.package");
    expect(state.targets).not.toHaveBeenCalled();
    expect(state.changesets).not.toHaveBeenCalled();
  });

  it("rollback 成功后刷新列表与时间线并返回 true", async () => {
    state.rollback.mockResolvedValue({
      outputPath: "C:/a.package",
      targetRevision: 1,
      newChangeset: summary(3),
      resourceCount: 1,
      bytesWritten: 10,
    });
    const store = useVersionConsoleStore();
    await store.openTarget("C:/a.package");
    state.targets.mockClear();
    state.changesets.mockClear();

    await expect(store.rollback(1, false)).resolves.toBe(true);

    expect(state.rollback).toHaveBeenCalledWith(1, false);
    expect(state.targets).toHaveBeenCalledTimes(1);
    expect(state.changesets).toHaveBeenCalledTimes(1);
  });

  it("外部漂移时 rollback 返回 false 并保留错误信息", async () => {
    state.rollback.mockRejectedValue(new Error("external_drift: 目标文件已被改动"));
    const store = useVersionConsoleStore();
    await store.openTarget("C:/a.package");

    await expect(store.rollback(1, false)).resolves.toBe(false);
    expect(store.error).toContain("external_drift");
  });

  it("remove 删除后刷新列表", async () => {
    state.remove.mockResolvedValue({ changesets: 1, items: 0, blobsReclaimed: 0 });
    const store = useVersionConsoleStore();
    await store.openTarget("C:/a.package");
    state.targets.mockClear();

    await store.remove(2);

    expect(state.remove).toHaveBeenCalledWith(2);
    expect(state.targets).toHaveBeenCalledTimes(1);
  });
});
