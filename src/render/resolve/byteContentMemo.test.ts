import { expect, it, vi } from "vitest";
import { createByteContentMemo } from "./byteContentMemo";

it("shares in-flight decoding for equal bytes but isolates sampler variants", async () => {
  const memo = createByteContentMemo<object>();
  const load = vi.fn(async () => ({}));
  const first = memo.get(new Uint8Array([1, 2, 3]), "data", load);
  const second = memo.get(new Uint8Array([1, 2, 3]), "data", load);
  const palette = memo.get(new Uint8Array([1, 2, 3]), "palette", load);
  expect(first).toBe(second);
  expect(await first).not.toBe(await palette);
  expect(load).toHaveBeenCalledTimes(2);
});

it("compares unsampled bytes before sharing and retries rejected loads", async () => {
  const memo = createByteContentMemo<number>();
  const a = new Uint8Array(128), b = a.slice();
  b[1] = 1; // sampled keys collide intentionally
  expect(await memo.get(a, "data", async () => 1)).toBe(1);
  expect(await memo.get(b, "data", async () => 2)).toBe(2);
  const failed = memo.get(a, "retry", async () => { throw Error("decode"); });
  await expect(failed).rejects.toThrow("decode");
  expect(await memo.get(a, "retry", async () => 3)).toBe(3);
});
