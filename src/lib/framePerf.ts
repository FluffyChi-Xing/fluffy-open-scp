/**
 * 帧级性能采样（模块级普通对象，不是 Vue ref）。
 *
 * 与 `renderTelemetry` 的分工：后者只记录异步装配阶段、刻意不进帧循环；
 * 这里只记录**真实出画帧**的 CPU 提交耗时、帧间隔与 `renderer.info`，
 * 用来回答"打开后一直掉帧/镜头不跟手"到底卡在哪一层。采样本身只有
 * 几次属性读取，不做 DOM、不上报、不触发响应式更新。
 */

export interface FramePerfSample {
  /** 本帧 render 调用占用的 CPU 时间（ms）。 */
  cpuMs: number;
  /** 与上一个出画帧的间隔（ms）；0 = 上一帧空闲过久，不计入 FPS。 */
  intervalMs: number;
  calls: number;
  triangles: number;
  programs: number;
  textures: number;
  geometries: number;
  dpr: number;
}

export interface FramePerfSnapshot {
  /** 采样窗口内的帧数。 */
  frames: number;
  /** 由帧间隔中位数换算的 FPS；无有效间隔 = 0。 */
  fps: number;
  /** 帧间隔 p50/p95（ms）。 */
  intervalP50: number;
  intervalP95: number;
  /** render CPU 提交耗时 p50/p95（ms）。 */
  cpuP50: number;
  cpuP95: number;
  calls: number;
  triangles: number;
  programs: number;
  textures: number;
  geometries: number;
  dpr: number;
  lights: number;
  /** WebGL 渲染器字符串（诊断软件渲染/核显）。 */
  gpu: string;
}

/** 保留的帧样本数（约 2 秒 @60fps）。 */
const WINDOW = 120;
/** 超过此间隔视为"场景静止后的第一帧"，不参与 FPS 统计。 */
const IDLE_GAP_MS = 250;

const samples: FramePerfSample[] = [];
let lights = 0;
let gpu = "";

function percentile(values: number[], ratio: number): number {
  if (!values.length) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.round((sorted.length - 1) * ratio)));
  return sorted[index];
}

export const framePerf = {
  recordFrame(
    cpuMs: number,
    intervalMs: number,
    info: Pick<FramePerfSample, "calls" | "triangles" | "programs" | "textures" | "geometries" | "dpr">,
  ): void {
    if (!Number.isFinite(cpuMs) || cpuMs < 0) return;
    samples.push({
      cpuMs,
      intervalMs: intervalMs > 0 && intervalMs < IDLE_GAP_MS ? intervalMs : 0,
      ...info,
    });
    if (samples.length > WINDOW) samples.splice(0, samples.length - WINDOW);
  },

  setLights(count: number): void {
    lights = count;
  },

  setGpu(label: string): void {
    gpu = label;
  },

  /** 近期快照（无样本时各统计值为 0）。 */
  snapshot(): FramePerfSnapshot {
    const last = samples[samples.length - 1];
    const intervals = samples.map((sample) => sample.intervalMs).filter((value) => value > 0);
    const cpu = samples.map((sample) => sample.cpuMs);
    const intervalP50 = percentile(intervals, 0.5);
    return {
      frames: samples.length,
      fps: intervalP50 > 0 ? 1000 / intervalP50 : 0,
      intervalP50,
      intervalP95: percentile(intervals, 0.95),
      cpuP50: percentile(cpu, 0.5),
      cpuP95: percentile(cpu, 0.95),
      calls: last?.calls ?? 0,
      triangles: last?.triangles ?? 0,
      programs: last?.programs ?? 0,
      textures: last?.textures ?? 0,
      geometries: last?.geometries ?? 0,
      dpr: last?.dpr ?? 0,
      lights,
      gpu,
    };
  },

  reset(): void {
    samples.length = 0;
    lights = 0;
  },
};
