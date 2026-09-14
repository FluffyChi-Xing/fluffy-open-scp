import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/** vi.mock 会被提升，状态必须用 vi.hoisted 定义。 */
const state = vi.hoisted(() => ({ tauri: true, record: vi.fn() }));

vi.mock("@/api", () => ({
  isTauri: () => state.tauri,
  tauriApi: { renderTelemetry: { record: state.record } },
}));

const { BUFFER_CAP, FLUSH_DELAY_MS, renderTelemetry } = await import("./renderTelemetry");

function span(stage: Parameters<typeof renderTelemetry.begin>[0]): void {
  renderTelemetry.begin(stage).end();
}

describe("renderTelemetry", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    state.tauri = true;
    state.record.mockReset();
    state.record.mockResolvedValue(1);
    renderTelemetry.clearRecent();
    renderTelemetry.setContext({ sessionKey: "s", trigger: "first_load" });
  });

  afterEach(async () => {
    // 先把待发批次排空：flushTimer 是模块级状态，假定时器切换后会残留在上一个
    // 测试的悬挂 id 上，导致下一个测试永远排不上批次。
    await vi.advanceTimersByTimeAsync(FLUSH_DELAY_MS);
    renderTelemetry.clearRecent();
    vi.useRealTimers();
  });

  it("批次合并：多次记录只上报一次", async () => {
    span("model_load");
    span("lot_render");
    span("decal_render");
    expect(state.record).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(FLUSH_DELAY_MS);
    expect(state.record).toHaveBeenCalledTimes(1);
    const batch = state.record.mock.calls[0][0] as unknown[];
    expect(batch).toHaveLength(3);
  });

  it("缓冲封顶，只保留最近 BUFFER_CAP 条", () => {
    for (let index = 0; index < BUFFER_CAP + 25; index += 1) {
      span("model_load");
    }
    expect(renderTelemetry.recent()).toHaveLength(BUFFER_CAP);
  });

  it("非 Tauri 环境整体 no-op（不记录、不上报）", async () => {
    state.tauri = false;
    span("model_load");
    await vi.advanceTimersByTimeAsync(FLUSH_DELAY_MS);
    expect(renderTelemetry.recent()).toHaveLength(0);
    expect(state.record).not.toHaveBeenCalled();
  });

  it("上报失败不抛出、不重试", async () => {
    state.record.mockRejectedValue(new Error("ipc down"));
    span("model_load");
    // 若 promise 未被 catch，这里会以未处理的 rejection 让测试失败。
    await vi.advanceTimersByTimeAsync(FLUSH_DELAY_MS);
    await Promise.resolve();
    expect(state.record).toHaveBeenCalledTimes(1);
    expect(renderTelemetry.recent()).toHaveLength(0);
  });

  it("trigger 随 setTrigger 变化并写入记录", async () => {
    renderTelemetry.setTrigger("render_mode");
    span("texture_compose");
    const [entry] = renderTelemetry.recent();
    expect(entry.trigger).toBe("render_mode");
    expect(entry.stage).toBe("texture_compose");
    expect(entry.sessionKey).toBe("s");
  });

  /** 性能报告（项目约定：每阶段测试附耗时）。用 hrtime 计时——假定时器接管了 performance.now。 */
  it("perf: 单次 begin/end 开销", () => {
    const runs = 10_000;
    const start = process.hrtime.bigint();
    for (let index = 0; index < runs; index += 1) {
      span("model_load");
    }
    const elapsedNs = Number(process.hrtime.bigint() - start);
    const perSpanUs = elapsedNs / 1000 / runs;
    console.log(
      `[perf renderTelemetry] ${runs} 次 begin/end：合计 ${(elapsedNs / 1e6).toFixed(1)}ms，单次 ${perSpanUs.toFixed(3)}µs`,
    );
    expect(perSpanUs).toBeLessThan(5);
    expect(renderTelemetry.recent()).toHaveLength(BUFFER_CAP);
  });
});

describe("不得侵入渲染帧循环（结构回归）", () => {
  const read = (relative: string) =>
    readFileSync(resolve(process.cwd(), "src/lib", relative), "utf8");

  it("three-viewer.ts 完全不引用记录器", () => {
    expect(read("three-viewer.ts")).not.toContain("renderTelemetry");
  });

  it("three-viewer.ts 的 renderLoop 内没有任何埋点调用", () => {
    const source = read("three-viewer.ts");
    const start = source.indexOf("renderLoop");
    expect(start).toBeGreaterThan(-1);
    // renderLoop 之后到下一个方法之间的窗口内不得出现记录器
    const window = source.slice(start, start + 1500);
    expect(window).not.toContain("renderTelemetry");
    expect(window).not.toContain("performance.now");
  });
});
