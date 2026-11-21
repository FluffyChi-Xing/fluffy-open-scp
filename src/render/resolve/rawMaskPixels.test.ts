import { afterEach, expect, it, vi } from "vitest";
import { createRawMaskDecoder } from "./rawMaskPixels";
afterEach(() => vi.unstubAllGlobals());
it("decodes original channel bytes once and invalidates on source or dimensions", () => {
  vi.stubGlobal("ImageData", class {
    constructor(public data: Uint8ClampedArray, public width: number, public height: number) {}
  });
  const decode = createRawMaskDecoder();
  const source = btoa(String.fromCharCode(255, 31, 63, 0, 1, 2, 3, 4));
  const first = decode(source, 2, 1)!;
  expect([...first.data]).toEqual([255, 31, 63, 0, 1, 2, 3, 4]);
  expect(decode(source, 2, 1)).toBe(first);
  expect(decode(source, 1, 2)).not.toBe(first);
  expect(decode(source.slice(0, 4), 2, 1)).toBeNull();
  expect(decode(null, 2, 1)).toBeNull();
});
