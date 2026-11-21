import type * as ThreeNamespace from "three";

export type DeferredDispose = (object: ThreeNamespace.Object3D) => void;

/** Bounded idle-time disposal queue used when replacing a large Three scene. */
export class DeferredDisposalQueue {
  private readonly queue: ThreeNamespace.Object3D[] = [];
  private scheduled = false;
  private paused = false;

  constructor(private readonly dispose: DeferredDispose) {}

  /** Keep old shader program references alive until replacement materials
   * have acquired them. Disposing during an assembly yield evicts hot programs. */
  setPaused(paused: boolean): void {
    this.paused = paused;
    if (!paused && this.queue.length) this.schedule();
  }

  enqueue(object: ThreeNamespace.Object3D): void {
    this.queue.push(object);
    this.schedule();
  }

  drain(): void {
    for (const object of this.queue.splice(0)) this.dispose(object);
    this.scheduled = false;
  }

  private schedule(): void {
    if (this.scheduled || this.paused) return;
    this.scheduled = true;
    const run = () => {
      if (this.paused) {
        this.scheduled = false;
        return;
      }
      const started = performance.now();
      while (this.queue.length && performance.now() - started < 4) {
        const object = this.queue.shift();
        if (object) this.dispose(object);
      }
      this.scheduled = false;
      if (this.queue.length) this.schedule();
    };
    const idle = (globalThis as typeof globalThis & {
      requestIdleCallback?: (callback: () => void) => number;
    }).requestIdleCallback;
    if (idle) idle(run);
    else setTimeout(run, 0);
  }
}
