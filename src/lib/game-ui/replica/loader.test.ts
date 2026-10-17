import { afterEach, describe, expect, it, vi } from "vitest";
import { loadReplicaData, mergeMenuCosts } from "./loader";

type ScriptHandler = (script: HTMLScriptElement) => void;

/** 拦截 head.appendChild：happy-dom 不会真的拉取脚本，由回调决定注入脚本是
 * 派发 load 还是 error（loader 依赖这两个事件推进 Promise）。 */
function interceptScripts(handler: ScriptHandler) {
  const spy = vi
    .spyOn(document.head, "appendChild")
    .mockImplementation(((node: Node) => {
      if (node instanceof HTMLScriptElement) {
        queueMicrotask(() => handler(node));
      }
      return node;
    }) as unknown as typeof document.head.appendChild);
  return spy;
}

function stubGlobals() {
  const globals = window as unknown as Record<string, unknown>;
  globals.HUD_LAYOUT = {};
  globals.CATEGORY_MODEL = { categories: [], row: {} };
  globals.HUD_PALETTE = {};
  globals.HUD_TOOLS = {};
}

afterEach(() => {
  vi.restoreAllMocks();
  const globals = window as unknown as Record<string, unknown>;
  delete globals.HUD_LAYOUT;
  delete globals.CATEGORY_MODEL;
  delete globals.HUD_PALETTE;
  delete globals.HUD_TOOLS;
});

describe("loadReplicaData", () => {
  it("装载失败后清空缓存：再次调用重新注入脚本而不是复用 rejected promise", async () => {
    // 第一轮：所有注入脚本都失败。
    let failing = true;
    const spy = interceptScripts((script) => {
      if (failing) script.dispatchEvent(new Event("error"));
      else script.dispatchEvent(new Event("load"));
    });
    await expect(loadReplicaData()).rejects.toThrow("复刻数据脚本加载失败");
    const callsAfterFailure = spy.mock.calls.length;
    expect(callsAfterFailure).toBeGreaterThan(0);

    // 第二轮：脚本恢复可加载，必须重新注入并成功——失败缓存已被清空。
    failing = false;
    stubGlobals();
    const data = await loadReplicaData();
    expect(spy.mock.calls.length).toBeGreaterThan(callsAfterFailure);
    expect(data.model.categories).toEqual([]);
  });

  it("装载成功后进程内缓存：再次调用不再注入脚本", async () => {
    const spy = interceptScripts((script) =>
      script.dispatchEvent(new Event("load")),
    );
    stubGlobals();
    await loadReplicaData();
    const callsAfterSuccess = spy.mock.calls.length;
    await loadReplicaData();
    expect(spy.mock.calls.length).toBe(callsAfterSuccess);
  });
});

describe("mergeMenuCosts", () => {
  it("只回填 cost/upkeep 为空的条目；已有值（如编辑覆盖前置值）不覆盖", () => {
    const tools = {
      CAT: [
        { instance: "05456823", label: "診所", preview: null, marquee: null, locked: false, cost: null, upkeep: null },
        { instance: "2C7473DE", label: "遊樂園人行道", preview: null, marquee: null, locked: false, cost: null, upkeep: null },
        { instance: "7DD8A627", label: "低密度泥路", preview: null, marquee: null, locked: false, cost: null, upkeep: null },
        { instance: "BBBB0002", label: "已有值", preview: null, marquee: null, locked: false, cost: "100", upkeep: "5" },
      ],
    };
    mergeMenuCosts(tools, {
      costs: {
        // 诊所：EcoGame Unit 条目（cToolPlop 链）
        "05456823": { cost: 20000, maintenance: 400, unit: "30236D71" },
        // 游乐园人行道：分段 pathCost=2
        "2C7473DE": { roadCost: 2, piece: "AEE71498" },
        // 低密度泥路：roadCost=0 = 引擎语义免费 → 显示 §0
        "7DD8A627": { roadCost: 0, piece: "0984F4ED" },
        // 已有值不覆盖
        BBBB0002: { cost: 999, maintenance: 999 },
      },
    });
    expect(tools.CAT[0].cost).toBe("20000");
    expect(tools.CAT[0].upkeep).toBe("400");
    expect(tools.CAT[1].cost).toBe("2");
    expect(tools.CAT[2].cost).toBe("0");
    expect(tools.CAT[3].cost).toBe("100");
    expect(tools.CAT[3].upkeep).toBe("5");
  });

  it("索引缺失/损坏时不抛错（costs 缺字段按空处理）", () => {
    const tools = { CAT: [{ instance: "AAAA0001", label: "x", preview: null, marquee: null, locked: false, cost: null, upkeep: null }] };
    expect(() => mergeMenuCosts(tools, {} as Parameters<typeof mergeMenuCosts>[1])).not.toThrow();
    expect(tools.CAT[0].cost).toBeNull();
  });
});
