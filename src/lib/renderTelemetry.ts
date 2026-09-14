/**
 * 渲染可观测性记录器 —— **被动单例**。
 *
 * 设计约束（「不干扰渲染引擎」）：
 * 1. **不进帧循环**：`three-viewer.ts` 的 `renderLoop` 刻意不埋点，只在已有的 async
 *    阶段边界（IPC / 图片解码 / 装配）取值。
 * 2. **只读时钟**：每个 span 只做一次 `performance.now()`，不读也不写任何 Three.js
 *    对象、uniform 或场景图。
 * 3. **I/O 全延后**：上报走 `setTimeout` 去抖批量，慢或失败都不阻塞装配；异常一律吞掉。
 * 4. **不与视口耦合**：缓冲区是模块级普通数组（不是 Vue ref），记录不会触发组件更新。
 *
 * 非 Tauri 环境（浏览器预览 / 单元测试）整体 no-op。
 */
import { isTauri, tauriApi } from "@/api";
import type { RenderTelemetryEntry } from "@/api/tauri";

export type RenderTelemetryStage =
  | "model_load"
  | "texture_compose"
  | "lot_render"
  | "decal_render"
  | "scene_rebuild";

export type RenderTelemetryTrigger =
  | "first_load"
  | "lod_switch"
  | "render_mode"
  | "grouping"
  | "scene_rebuild";

export interface RenderSpan {
  /** 结束计时并记录一条。`metadata` 会与 `begin` 时传入的合并（后者优先）。 */
  end(metadata?: Record<string, unknown>): void;
}

/** 面板最多展示的实时样本数。 */
export const BUFFER_CAP = 200;
/** 上报去抖窗口。 */
export const FLUSH_DELAY_MS = 250;

const buffer: RenderTelemetryEntry[] = [];
let sessionKey = "";
let trigger: RenderTelemetryTrigger = "first_load";
let flushTimer: ReturnType<typeof setTimeout> | null = null;

const NOOP_SPAN: RenderSpan = { end() {} };

function record(
  stage: RenderTelemetryStage,
  durationMs: number,
  metadata?: Record<string, unknown>,
): void {
  buffer.push({
    sessionKey,
    stage,
    trigger,
    durationMs,
    ...(metadata ? { metadata } : {}),
  });
  if (buffer.length > BUFFER_CAP) {
    buffer.splice(0, buffer.length - BUFFER_CAP);
  }
  scheduleFlush();
}

function scheduleFlush(): void {
  if (flushTimer !== null) return;
  flushTimer = setTimeout(() => {
    flushTimer = null;
    if (buffer.length === 0) return;
    const batch = buffer.splice(0, buffer.length);
    // 上报失败绝不影响渲染路径，也不重试（下一次触发会继续累积）。
    void tauriApi.renderTelemetry.record(batch).catch(() => {});
  }, FLUSH_DELAY_MS);
}

export const renderTelemetry = {
  /** 记录器是否生效（仅 Tauri 运行时）。 */
  enabled(): boolean {
    return isTauri();
  },

  /**
   * 设置当前渲染触发的上下文。应在 `rebuildScene()` 顶部调用，
   * 之后的所有 span 都会带上该 session / trigger。
   */
  setContext(next: { sessionKey: string; trigger: RenderTelemetryTrigger }): void {
    sessionKey = next.sessionKey;
    trigger = next.trigger;
  },

  /**
   * 只改 trigger（保留当前 sessionKey）。视口重建时用得到——它不需要知道
   * packageId/tgi，会话键已由 `usePropertyEditorSession` 设好。
   */
  setTrigger(next: RenderTelemetryTrigger): void {
    trigger = next;
  },

  currentTrigger(): RenderTelemetryTrigger {
    return trigger;
  },

  /** 开始一个阶段计时。非 Tauri 环境返回 no-op span。 */
  begin(
    stage: RenderTelemetryStage,
    metadata?: Record<string, unknown>,
  ): RenderSpan {
    if (!isTauri()) return NOOP_SPAN;
    const started = performance.now();
    return {
      end(extra?: Record<string, unknown>): void {
        const merged =
          metadata || extra ? { ...metadata, ...extra } : undefined;
        record(stage, performance.now() - started, merged);
      },
    };
  },

  /** 实时样本（只读视图，供遥测页签渲染）。 */
  recent(): readonly RenderTelemetryEntry[] {
    return buffer;
  },

  clearRecent(): void {
    buffer.length = 0;
  },
};
