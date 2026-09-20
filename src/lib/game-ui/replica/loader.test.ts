import { afterEach, describe, expect, it, vi } from "vitest";
import { loadReplicaData } from "./loader";

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
