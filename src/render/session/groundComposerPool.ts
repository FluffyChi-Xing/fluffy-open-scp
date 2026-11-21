import { groundOutputSize, type GroundComposeInput, type GroundComposeResponse } from "../resolve/groundCompose";

/** Independent output rows use global UV/derivative coordinates. Each worker
 * returns only its stripe; the host joins exact bytes, without resampling. */
export function createGroundComposerPool(createWorker: () => Worker, concurrency: number) {
  const workers: Worker[] = [];
  const jobs = new Map<number, { resolve: (output: GroundComposeResponse) => void; reject: (error: Error) => void }>();
  let nextId = 0, broken = false;
  function fail() {
    broken = true;
    for (const worker of workers) worker.terminate();
    workers.length = 0;
    for (const job of jobs.values()) job.reject(new Error("ground worker unavailable"));
    jobs.clear();
  }
  function prepare() {
    if (broken) return;
    try {
      while (workers.length < Math.max(1, Math.min(4, concurrency))) {
        const worker = createWorker();
        worker.onmessage = (event: MessageEvent<GroundComposeResponse & { error?: string }>) => {
          if (event.data.error) { fail(); return; }
          const job = jobs.get(event.data.id);
          jobs.delete(event.data.id);
          job?.resolve(event.data);
        };
        worker.onerror = fail;
        workers.push(worker);
      }
    } catch { fail(); }
  }
  return {
    prepare,
    async compose(input: GroundComposeInput, transfers: ArrayBuffer[]): Promise<GroundComposeResponse> {
      prepare();
      if (broken || !workers.length) throw new Error("ground worker unavailable");
      const size = input.outSize ?? groundOutputSize(input.mask.width, input.mask.height, input.tilesX, input.tilesY, 0);
      const count = size.width * size.height >= 512 * 512 ? Math.min(workers.length, size.height) : 1;
      const stripes: Promise<GroundComposeResponse>[] = [];
      for (let i = 0; i < count; i++) {
        const rows = [Math.floor(i * size.height / count), Math.floor((i + 1) * size.height / count)] as const;
        stripes.push(new Promise((resolve, reject) => {
          const id = ++nextId;
          jobs.set(id, { resolve, reject });
          try {
            // Parallel jobs need independent input copies. Transfer only when
            // one worker owns the input; never detach a shared cached atlas.
            workers[i].postMessage({ id, input, rows }, count === 1 ? transfers : []);
          } catch { fail(); }
        }));
        if (broken) break;
      }
      const parts = await Promise.all(stripes);
      if (parts.length === 1) return parts[0];
      const albedo = new Uint8ClampedArray(size.width * size.height * 4);
      const normal = new Uint8ClampedArray(albedo.length);
      let offset = 0;
      for (const part of parts) {
        albedo.set(part.albedo, offset); normal.set(part.normal, offset);
        offset += part.albedo.length;
      }
      return { id: -1, width: size.width, height: size.height, albedo, normal };
    },
  };
}
