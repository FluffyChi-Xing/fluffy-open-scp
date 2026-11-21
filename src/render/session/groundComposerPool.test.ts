import { expect, it, vi } from "vitest";
import type { GroundComposeInput, GroundComposeRequest, GroundComposeResponse } from "../resolve/groundCompose";
import { createGroundComposerPool } from "./groundComposerPool";

function setup() {
  const workers: {
    onmessage: ((event: MessageEvent<GroundComposeResponse>) => void) | null;
    onerror: (() => void) | null;
    postMessage: ReturnType<typeof vi.fn>;
    terminate: ReturnType<typeof vi.fn>;
  }[] = [];
  const pool = createGroundComposerPool(() => {
    const worker = { onmessage: null, onerror: null, postMessage: vi.fn(), terminate: vi.fn() };
    workers.push(worker);
    return worker as unknown as Worker;
  }, 4);
  return { pool, workers };
}

it("joins out-of-order odd-sized stripes in exact row order", async () => {
  const { pool, workers } = setup();
  const input = { outSize: { width: 512, height: 513 } } as GroundComposeInput;
  const pending = pool.compose(input, []);
  expect(workers).toHaveLength(4);
  for (const [index, worker] of [...workers.entries()].reverse()) {
    const request = worker.postMessage.mock.calls[0][0] as GroundComposeRequest;
    const [start, end] = request.rows!;
    expect(worker.postMessage.mock.calls[0][1]).toEqual([]);
    worker.onmessage?.({ data: {
      id: request.id, width: 512, height: end - start,
      albedo: new Uint8ClampedArray(512 * (end - start) * 4).fill(index + 1),
      normal: new Uint8ClampedArray(512 * (end - start) * 4).fill(index + 5),
    } } as MessageEvent<GroundComposeResponse>);
  }
  const result = await pending;
  expect(result.albedo.length).toBe(512 * 513 * 4);
  for (let y = 0; y < 513; y++) {
    const index = workers.findIndex(worker => {
      const [start, end] = worker.postMessage.mock.calls[0][0].rows;
      return y >= start && y < end;
    });
    expect(result.albedo[y * 512 * 4]).toBe(index + 1);
    expect(result.normal[y * 512 * 4]).toBe(index + 5);
  }
});

it("rejects every pending stripe and terminates the pool on worker failure", async () => {
  const { pool, workers } = setup();
  const pending = pool.compose({ outSize: { width: 512, height: 512 } } as GroundComposeInput, []);
  const rejection = expect(pending).rejects.toThrow("unavailable");
  workers[1].onerror?.();
  await rejection;
  for (const worker of workers) expect(worker.terminate).toHaveBeenCalledOnce();
  await expect(pool.compose({} as GroundComposeInput, [])).rejects.toThrow("unavailable");
});
