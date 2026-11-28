interface PixelRatioHost {
  getPixelRatio(): number;
  setPixelRatio(ratio: number): void;
}

/** A backing-buffer resize clears WebGL pixels. Queue it until a synchronous
 * draw can refill the buffer, never after presenting a completed frame. */
export function createFramePixelRatio() {
  let pending: number | null = null;
  return {
    request(ratio: number): void { pending = ratio; },
    desired(fallback: number): number { return pending ?? fallback; },
    draw(host: PixelRatioHost, render: () => void): void {
      const ratio = pending;
      pending = null;
      if (ratio !== null && ratio !== host.getPixelRatio()) host.setPixelRatio(ratio);
      render();
    },
  };
}
