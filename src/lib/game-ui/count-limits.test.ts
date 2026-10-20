import { beforeEach, describe, expect, it, vi } from "vitest";
import { useCountLimits } from "./count-limits";
import manifest from "../../../public/game-ui/tools/count-limits.json";

const mocks = vi.hoisted(() => ({ desktop: false, command: vi.fn() }));
vi.mock("@/api", () => ({ isTauri: () => mocks.desktop }));
vi.mock("@/api/tauri", () => ({ command: mocks.command }));

describe("building count edits", () => {
  beforeEach(() => {
    mocks.desktop = false;
    mocks.command.mockReset();
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => manifest }));
  });

  it("detects capabilities and shares the group edit without changing another individual limit", async () => {
    const limits = useCountLimits();
    expect(await limits.load("BC45E41B")).toEqual({ individual: 1, shared: null, sharedKey: null });
    expect(await limits.load("C0E5BE77")).toEqual({ individual: 1, shared: 3, sharedKey: "4B427C97" });
    await limits.load("330B54A5");
    limits.apply({ source: "C0E5BE77", individual: 2, shared: 5 });
    expect(limits.effective("330B54A5")).toMatchObject({ individual: 1, shared: 5 });
    expect((await limits.load("FFFFFFFF")).individual).toBeNull();
    expect(limits.editCount.value).toBe(2);
    limits.apply({ source: "330B54A5", individual: 1, shared: 3 });
    expect(limits.effective("C0E5BE77")).toMatchObject({ individual: 2, shared: 3 });
    limits.reset();
    expect(limits.editCount.value).toBe(0);
  });

  it("rejects invalid input atomically and refuses an incomplete browser package", async () => {
    const limits = useCountLimits();
    await limits.load("C0E5BE77");
    for (const value of ["", 0, -1, 1.5, NaN, Infinity, 16777217]) {
      expect(() => limits.apply({ source: "C0E5BE77", individual: 2, shared: value })).toThrow();
      expect(limits.editCount.value).toBe(0);
    }
    limits.apply({ source: "C0E5BE77", individual: 2, shared: 4 });
    await expect(limits.compile([])).rejects.toThrow();
    expect(limits.editCount.value).toBe(2);
  });

  it("batches edits and forwards existing resource bytes so economics survive the count patch", async () => {
    const limits = useCountLimits();
    await limits.load("C0E5BE77");
    await limits.load("330B54A5");
    limits.apply({ source: "C0E5BE77", individual: 2, shared: 5 });
    limits.apply({ source: "330B54A5", individual: 1, shared: 6 });
    mocks.desktop = true;
    mocks.command.mockResolvedValue([{ typeId: 0xb1b104, group: 2, instance: 3, dataBase64: "BAU=" }]);
    const resources = await limits.compile([{ type: 0xb1b104, group: 2, instance: 3, data: new Uint8Array([1, 2, 3]) }]);
    expect(mocks.command).toHaveBeenCalledWith("game_count_limits_patch", {
      changes: [
        { sourceInstance: 0xc0e5be77, individual: 2 },
        { sourceInstance: 0x330b54a5, shared: 6 },
      ],
      baseResources: [{ typeId: 0xb1b104, group: 2, instance: 3, dataBase64: "AQID" }],
    });
    expect([...resources[0].data]).toEqual([4, 5]);
  });
});
