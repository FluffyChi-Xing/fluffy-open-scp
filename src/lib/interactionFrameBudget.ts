/** Per-view motion budget. Pixel count scales quadratically with the ratio.
 * Only sustained slow frames lower it; idle gaps and one-off spikes do not.
 * Resolution is held for the gesture to avoid oscillating image sharpness. */
export function createInteractionFrameBudget() {
  let ratio = 1;
  let minimum = 0.5;
  let slowFrames = 0;
  return {
    begin(initialRatio: number): void {
      ratio = initialRatio;
      minimum = Math.min(initialRatio, 0.5);
      slowFrames = 0;
    },
    sample(cpuMs: number, intervalMs: number): number {
      // First frame after a stationary scene is not a frame-rate sample.
      const interval = intervalMs > 0 && intervalMs <= 200 ? intervalMs : 0;
      const cost = Math.max(cpuMs, interval);
      if (!Number.isFinite(cost) || cost <= 24) {
        slowFrames = 0;
        return ratio;
      }
      slowFrames++;
      if (slowFrames >= 3) {
        // A heavily overloaded GPU needs one decisive resize, rather than
        // reallocating the backing buffer at three intermediate sizes.
        ratio = Math.max(minimum, ratio * (cost >= 40 ? 0.5 : 0.8));
        slowFrames = 0;
      }
      return ratio;
    },
  };
}
