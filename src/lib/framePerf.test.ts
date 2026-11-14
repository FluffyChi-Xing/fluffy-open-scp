import { beforeEach, describe, expect, it } from "vitest";
import { framePerf } from "./framePerf";

const info = {
  calls: 120,
  triangles: 34000,
  programs: 9,
  textures: 40,
  geometries: 12,
  dpr: 1.5,
};

describe("framePerf", () => {
  beforeEach(() => framePerf.reset());

  it("reports FPS from the median rendered-frame interval", () => {
    framePerf.recordFrame(4, 16, info);
    framePerf.recordFrame(4, 16, info);
    framePerf.recordFrame(4, 33, info);
    const snapshot = framePerf.snapshot();
    expect(snapshot.frames).toBe(3);
    expect(snapshot.intervalP50).toBe(16);
    expect(snapshot.fps).toBeCloseTo(62.5, 5);
    expect(snapshot.intervalP95).toBe(33);
    expect(snapshot.cpuP50).toBe(4);
  });

  it("drops idle gaps so a static scene does not report 0 FPS", () => {
    framePerf.recordFrame(2, 900, info);
    const snapshot = framePerf.snapshot();
    expect(snapshot.frames).toBe(1);
    expect(snapshot.intervalP50).toBe(0);
    expect(snapshot.fps).toBe(0);
  });

  it("keeps the latest renderer info and the light count", () => {
    framePerf.setLights(16);
    framePerf.setGpu("ANGLE (NVIDIA)");
    framePerf.recordFrame(2, 16, info);
    framePerf.recordFrame(3, 16, { ...info, calls: 99, dpr: 1 });
    const snapshot = framePerf.snapshot();
    expect(snapshot.calls).toBe(99);
    expect(snapshot.dpr).toBe(1);
    expect(snapshot.lights).toBe(16);
    expect(snapshot.gpu).toBe("ANGLE (NVIDIA)");
  });
});
