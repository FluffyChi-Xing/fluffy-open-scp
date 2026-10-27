import type * as ThreeNamespace from "three";

/**
 * 摄像机工作台景深控制器(独立模块,遵循 2026-10-27 代码纪律:语义内聚、
 * 依赖显式注入、副作用只在生命周期边界)。仅摄像机工作台链路启用——
 * 启用后 renderLoop 经 EffectComposer(RenderPass + BokehPass)出画;
 * 未启用时经 render(fallback) 走直渲,其他场景零开销零风险。
 *
 * 参数换算(游戏口径 → 场景):
 * - distance(游戏米)由调用方注入的 getFrameRadius 按 scenePerGame 换算
 *   为对焦距离(场景单位,与相机同一比例尺);
 * - aperture(游戏光圈 0.7~16)按 APERTURE_SCALE 换算 Bokeh 斜率;
 * - softness(0~1)映射最大模糊半径。
 */

export interface CameraFocusParams {
  /** 对焦距离(游戏米)。 */
  distance: number;
  /** 光圈(游戏口径)。 */
  aperture: number;
  /** 柔化 0~1。 */
  softness: number;
}

/** 光圈标定常数:游戏光圈 1.0 的 Bokeh 斜率(场景单位/视深)。 */
const APERTURE_SCALE = 0.0008;
/** 柔化→最大模糊半径:softness 0.45(默认)≈ 0.011,1.0 ≈ 0.015。 */
const MAXBLUR_BASE = 0.008;
const MAXBLUR_GAIN = 0.007;

export interface DepthOfFieldController {
  /** 设置工作台景深参数;null = 关闭(释放 composer,恢复直渲)。 */
  setGameParams(params: CameraFocusParams | null): void;
  /** 渲染一帧:启用时经 composer 出画,否则走 fallback 直渲。 */
  render(fallback: () => void): void;
  setSize(width: number, height: number): void;
  dispose(): void;
}

export function createDepthOfFieldController(deps: {
  renderer: () => ThreeNamespace.WebGLRenderer;
  scene: () => ThreeNamespace.Scene;
  camera: () => ThreeNamespace.Camera;
  getFrameRadius: () => number;
  isDisposed: () => boolean;
}): DepthOfFieldController {
  type Rig = {
    render(): void;
    setSize(width: number, height: number): void;
    setPixelRatio(ratio: number): void;
    setParams(params: { focus: number; aperture: number; softness: number }): void;
    dispose(): void;
  };
  let rig: Rig | null = null;
  let pending: { focus: number; aperture: number; softness: number } | null = null;
  let disposed = false;

  const toSceneFocus = (distance: number) =>
    distance * ((2.5 * Math.max(deps.getFrameRadius(), 1e-3)) / 800);

  function ensureRig() {
    if (rig || disposed) return;
    void (async () => {
      const [{ EffectComposer }, { RenderPass }, { BokehPass }, { OutputPass }] = await Promise.all([
        import("three/examples/jsm/postprocessing/EffectComposer.js"),
        import("three/examples/jsm/postprocessing/RenderPass.js"),
        import("three/examples/jsm/postprocessing/BokehPass.js"),
        import("three/examples/jsm/postprocessing/OutputPass.js"),
      ]);
      const instance = new EffectComposer(deps.renderer());
      instance.addPass(new RenderPass(deps.scene(), deps.camera()));
      const bokeh = new BokehPass(deps.scene(), deps.camera(), {
        focus: 1,
        aperture: 0.0008,
        maxblur: MAXBLUR_BASE + MAXBLUR_GAIN * 0.45,
      });
      instance.addPass(bokeh);
      // composer 内部缓冲是线性空间:链尾必须 OutputPass 补 renderer 的
      // toneMapping + 线性→sRGB 输出转换,否则整帧发黑(直渲时由
      // renderer.render 到画布自动完成,composer 不自动做)。
      instance.addPass(new OutputPass());
      if (disposed || deps.isDisposed()) {
        instance.dispose();
        return;
      }
      rig = {
        render: () => instance.render(),
        setSize: (width, height) => instance.setSize(width, height),
        setPixelRatio: (ratio) => instance.setPixelRatio(ratio),
        setParams: (p) => {
          const uniforms = bokeh.uniforms as unknown as Record<
            "focus" | "aperture" | "maxblur",
            { value: number }
          >;
          uniforms["focus"].value = p.focus;
          uniforms["aperture"].value = Math.max(
            1e-6,
            p.aperture * APERTURE_SCALE,
          );
          uniforms["maxblur"].value = Math.max(
            0,
            MAXBLUR_BASE + MAXBLUR_GAIN * p.softness,
          );
        },
        dispose: () => instance.dispose(),
      };
      rig.setPixelRatio(deps.renderer().getPixelRatio());
      if (pending) rig.setParams(pending);
    })();
  }

  return {
    setGameParams(params) {
      pending = params
        ? {
            focus: toSceneFocus(params.distance),
            aperture: params.aperture,
            softness: params.softness,
          }
        : null;
      if (!pending) {
        rig?.dispose();
        rig = null;
        return;
      }
      if (rig) rig.setParams(pending);
      else ensureRig();
    },
    render(fallback) {
      if (rig) rig.render();
      else fallback();
    },
    setSize(width, height) {
      rig?.setSize(width, height);
    },
    dispose() {
      disposed = true;
      rig?.dispose();
      rig = null;
    },
  };
}
