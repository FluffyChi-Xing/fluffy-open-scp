import { onBeforeUnmount, onMounted, ref, shallowRef } from "vue";
import { ThreeViewer } from "@/lib/three-viewer";
import { renderTelemetry } from "@/lib/renderTelemetry";
import type * as ThreeNamespace from "three";

/** 场景图层组注册表：底座只认组名，组的业务语义由装配层定义。 */
export const VIEWPORT_GROUPS = [
  "model",
  "lot",
  "dimensions",
  "lights",
  "props",
  "decals",
  "effects",
  "spawners",
  "paths",
] as const;
export type ViewportGroupName = (typeof VIEWPORT_GROUPS)[number];

/**
 * 取景内容组：建筑/地块/尺度标注。单元 gizmo 组（lights/props/decals/
 * effects/spawners/paths 的标记锥与垫片）是编辑器 chrome——其数据高度
 * （道具锚点沉到地块下数米、贴花垫片浮在上方）不参与取景与地面网格
 * 锚定，「网格参照物」与「编辑器标记」在此分离（2026-10-27 取景取证
 * 结论：网格贴全内容最低点时，编辑器标记把地面拖离建筑底面）。
 */
export const FRAMING_GROUPS = ["model", "lot", "dimensions"] as const;

/**
 * 单次 rebuild 的装配上下文：装配层（scene contributor）在回调内完成
 * 业务场景搭建，底座负责前后清理、防竞态与收尾。
 */
export interface EditorViewportRebuildCtx {
  prepareStableShaders: () => Promise<void>;
  viewer: ThreeViewer;
  THREE: typeof ThreeNamespace;
  token: number;
  /** 异步装配过程中重建是否已被新一代取代（取代后应放弃并回收资源）。 */
  isStale(): boolean;
  /** 登记本轮创建的贴图 blob URL，rebuild 时统一回收。 */
  registerTextureUrl(url: string): void;
  /** 硬件最大各向异性过滤级别（贴图加载时透传）。 */
  maxAnisotropy: number;
  /** unitId → Object3D 拾取/可见性/选中注册表（装配层写入）。 */
  unitObjects: Map<string, ThreeNamespace.Object3D>;
  /** 装配层回填的规模统计（进 scene_rebuild 遥测 metadata，量化
   *  「property 规模 ↔ 阶段耗时」相关性）。 */
  stats: Record<string, unknown>;
}

/** 悬停拾取的目标组（地面/建筑模型除外——它们不是 Unit，不参与 hover/选中框）。 */
export const PICKABLE_GROUPS = [
  "lights",
  "props",
  "decals",
  "effects",
  "spawners",
  "paths",
] as const;

/**
 * 编辑器视口底座：viewer 生命周期、拾取→select、rebuild 骨架（清组/
 * 防竞态/贴图 URL 回收/收尾构图）、可见性与选中应用、渲染截图。
 * 与使用形态无关（预览/property 编辑/rw4 编辑共用），业务场景装配通过
 * rebuild(assembly) 回调注入。
 */
export function useEditorViewport(options: {
  maxPixelRatio?: number;
  interactionPixelRatio?: number;
  onTapUnit: (id: string | null) => void;
  onHoverUnit?: (id: string | null) => void;
  /**
   * 清场前钩子：装配层在此摘除指向即将被移除对象的编辑器挂件
   * （TransformControls 等）。否则挂件在清场后仍持有已脱离场景图的对象，
   * three 每次 updateMatrixWorld 都会报 "attached object must be a part of
   * the scene graph"（2026-11-06 用户控制台刷屏）。
   */
  onBeforeClear?: () => void;
}) {
  const container = shallowRef<HTMLElement | null>(null);
  const viewer = shallowRef<ThreeViewer | null>(null);
  const sceneReady = ref(false);
  /** rebuild 代数：每轮装配完成 +1（选中对象被重建后手柄等需据此重挂）。 */
  const revision = ref(0);
  const unitObjects = new Map<string, ThreeNamespace.Object3D>();
  const textureUrls: string[] = [];
  /** 是否已构图过（首次构图后保持相机，编辑操作不再重置镜头）。 */
  let framed = false;
  let rebuildToken = 0;
  /** 重建在飞标记：资产通道部分刷新据此让位（见 isAssembling）。 */
  let assembling = false;
  let disposed = false;
  /** 在飞重建完全落地的等待者（whenSettled；尾部统一 flush）。 */
  const rebuildWaiters: (() => void)[] = [];
  let readyResolve: (() => void) | null = null;
  /** viewer 创建完成信号（装配层 onMounted 后据此触发首次 rebuild）。 */
  const ready = new Promise<void>((resolve) => {
    readyResolve = resolve;
  });

  /** 近距离兜底半径（px）：小标记 gizmo 射线难命中，光标附近最近的
   * unit 锚点直接当选——「PE 难以选中原件」的主修手段。 */
  const PROXIMITY_PX = 14;
  /** 命中对象 → unitId：沿父链找 userData.unitId；未命中时以光标坐标做
   * 近距离兜底（event 为 null = hover 场景，raycast 已含全部目标，不兜底）。 */
  function resolveUnitId(
    object: ThreeNamespace.Object3D | null,
    event: { clientX: number; clientY: number } | null,
  ): string | null {
    let node = object;
    while (node) {
      if (typeof node.userData?.unitId === "string") return node.userData.unitId;
      node = node.parent;
    }
    if (!event || !viewer.value) return null;
    const instance = viewer.value;
    const rect = container.value?.getBoundingClientRect();
    if (!rect) return null;
    const pointerX = event.clientX - rect.left;
    const pointerY = event.clientY - rect.top;
    const anchor = new instance.THREE.Vector3();
    let best: string | null = null;
    let bestDist = PROXIMITY_PX;
    for (const [id, unitObject] of unitObjects) {
      if (!unitObject.visible) continue;
      unitObject.getWorldPosition(anchor);
      const screen = instance.worldToScreen(anchor);
      if (screen.behind) continue;
      const dist = Math.hypot(screen.x - pointerX, screen.y - pointerY);
      if (dist < bestDist) {
        bestDist = dist;
        best = id;
      }
    }
    return best;
  }

  /** 模型组是否参与拾取（mod 工作台草稿：导入的建筑模型可作为伪单元
   * "model" 选中/变换）。切换时重设 hover 射线目标组。 */
  let modelPickable = false;
  function applyHoverTargets() {
    const instance = viewer.value;
    if (!instance) return;
    const targets = PICKABLE_GROUPS.map((name) => instance.group(name));
    if (modelPickable) targets.push(instance.group("model"));
    instance.setHoverTargets(targets);
  }
  function setModelPickable(pickable: boolean) {
    modelPickable = pickable;
    applyHoverTargets();
  }

  onMounted(async () => {
    const element = container.value;
    if (!element) return;
    viewer.value = await ThreeViewer.create(element, {
      maxPixelRatio: options.maxPixelRatio,
      interactionPixelRatio: options.interactionPixelRatio,
      onTap: (hit) => {
        options.onTapUnit(resolveUnitId(hit.object, hit.event));
      },
      onHover: (hit) => {
        options.onHoverUnit?.(hit ? resolveUnitId(hit.object, null) : null);
      },
    });
    // 悬停拾取只对 unit 图层组（建筑/地面命中不属于任何 Unit；
    // setModelPickable 可把模型组临时纳入）
    applyHoverTargets();
    readyResolve?.();
  });
  onBeforeUnmount(() => {
    disposed = true;
    rebuildToken += 1;
    assembling = false;
    for (const waiter of rebuildWaiters.splice(0)) waiter();
    viewer.value?.dispose();
    viewer.value = null;
    unitObjects.clear();
    for (const url of textureUrls) URL.revokeObjectURL(url);
    textureUrls.length = 0;
  });

  /**
   * 场景真实光源总数上限：three 前向渲染**每片元**评估全部 Point/Spot
   * 光源（成本 = 光源数 × 片元数 × BRDF）。48 盏在 2× DPR 下会把精细模式
   * 的片元成本推到无法跟手（2026-11-06 用户实测）。12 保留最强的招牌/
   * 路灯照明，其余剪枝；多灯 lot 若需要更多，先提升交互降级力度再调此值。
   * 剪枝策略按单元总强度从弱到强。
   */
  const MAX_REAL_LIGHTS = 12;

  /** 真实光源总数超限时，从强度最弱的单元开始摘除光源本体。 */
  function pruneExcessLights() {
    const perUnit: {
      lights: ThreeNamespace.Light[];
      intensity: number;
    }[] = [];
    let total = 0;
    for (const [id, object] of unitObjects) {
      if (!id.startsWith("light:")) continue;
      const lights: ThreeNamespace.Light[] = [];
      object.traverse((child) => {
        if ((child as ThreeNamespace.Light).isLight) {
          lights.push(child as ThreeNamespace.Light);
        }
      });
      if (!lights.length) continue;
      total += lights.length;
      perUnit.push({
        lights,
        intensity: lights.reduce((sum, light) => sum + light.intensity, 0),
      });
    }
    if (total <= MAX_REAL_LIGHTS) return;
    perUnit.sort((a, b) => a.intensity - b.intensity);
    for (const entry of perUnit) {
      for (const light of entry.lights) {
        if (total <= MAX_REAL_LIGHTS) return;
        light.parent?.remove(light);
        light.dispose();
        total -= 1;
      }
    }
  }

  /**
   * 清理旧场景并执行业务装配；收尾做光源裁剪，构图仅在首次或显式
   * 要求时执行（避免每次编辑操作都重置镜头，reframe 用于模型/LOD 更换）。
   */
  async function rebuild(
    assembly: (ctx: EditorViewportRebuildCtx) => Promise<void> | void,
    rebuildOptions: { reframe?: boolean; prepareGpu?: boolean } = {},
  ) {
    const instance = viewer.value;
    if (!instance) return;
    const token = ++rebuildToken;
    assembling = true;
    instance.setSceneAssembling(true);
    // 先摘挂件再清场：挂件（gizmo）持有上一代对象，清场后对象 parent=null，
    // 后续任何 scene.updateMatrixWorld 都会触发 three 的告警刷屏。
    try {
      options.onBeforeClear?.();
    } catch {
      // 挂件清理失败不应阻断重建
    }
    for (const name of VIEWPORT_GROUPS) instance.clearGroup(name);
    unitObjects.clear();
    for (const url of textureUrls) URL.revokeObjectURL(url);
    textureUrls.length = 0;
    const ctx: EditorViewportRebuildCtx = {
      prepareStableShaders: () => {
        pruneExcessLights();
        return instance.prepareGroupShaders(["model", "props", "spawners"], () => token !== rebuildToken);
      },
      viewer: instance,
      THREE: instance.THREE,
      token,
      isStale: () => token !== rebuildToken,
      registerTextureUrl: (url) => textureUrls.push(url),
      maxAnisotropy: instance.maxAnisotropy,
      unitObjects,
      stats: {},
    };
    const span = renderTelemetry.begin("scene_rebuild", {
      reframe: rebuildOptions.reframe === true,
      first: !framed,
    });
    try {
      await assembly(ctx);
      if (token !== rebuildToken) return;
      pruneExcessLights();
      if (rebuildOptions.reframe || !framed) {
        instance.frameContent({ framingGroups: [...FRAMING_GROUPS] });
        framed = true;
      } else {
        // 软跟随：渐进装配（模型/lotPlacement/草稿载荷异步到达）之后，
        // 首帧构图不能代表最终内容——不跟随的话地面网格与 content 归零
        // 停在首帧坐标系，建筑观感上"飘在网格上方"（2026-10-27 回归根因）。
        instance.frameContent({
          soft: true,
          framingGroups: [...FRAMING_GROUPS],
        });
      }
      sceneReady.value = true;
      revision.value += 1;
      // GPU 侧准备：**先放行首帧**，未上屏的纹理再按 idle 切片补传。
      // 此前"全量 primeTextures + 整场 compile 完成后才出画"会把首帧
      // 推迟数百 ms（模式切换卡死），而放到首帧后同步跑同样是主线程冻帧。
      if (token === rebuildToken) {
        // 首帧先放行；GPU 预热放到首帧之后的 idle 切片。
        instance.setSceneAssembling(false);
        instance.invalidate();
        if (rebuildOptions.prepareGpu === true) {
          // 逐纹理上传、每片 ~6ms：整场 prime 在 refined 首帧后同步跑完会
          // 把刚刚可见的视口再冻住数百毫秒（用户观测的「切换精细卡死」）。
          // 首帧本身需要的纹理由 draw 惰性上传，这里只补未上屏的部分。
          instance.primeTexturesInBackground(() => token !== rebuildToken);
        }
      }
    } finally {
      // 仅当前代清旗/唤醒：过期代误清会抹掉新一代的在飞标记
      if (token === rebuildToken) {
        assembling = false;
        instance.setSceneAssembling(false);
        for (const waiter of rebuildWaiters.splice(0)) waiter();
      }
      span.end(ctx.stats);
    }
  }

  function applyGroupVisibility(map: Record<string, boolean>) {
    const instance = viewer.value;
    if (!instance) return;
    for (const name of VIEWPORT_GROUPS) {
      instance.group(name).visible = map[name] !== false;
    }
    instance.invalidate();
  }

  function applyUnitVisibility(hidden: Set<string>) {
    for (const [id, object] of unitObjects) {
      object.visible = !hidden.has(id);
    }
    viewer.value?.invalidate();
  }

  function applySelection(selectedId: string | null) {
    const selected = selectedId ? unitObjects.get(selectedId) ?? null : null;
    viewer.value?.setSelected(selected);
  }

  /** 渲染图导出：剔除 effects/spawners/paths 等 gizmo 组（保留模型、灯光与
   * props——P2 起 props 精细模式为真实车辆/模型、默认模式为标记锥，均属
   * 场景内容，截图默认保留，2026-10-05 用户指令），返回 PNG dataURL；
   * null = 截图失败。
   *
   * `includeDecals`：精细模式下 decal 已是投影到建筑面的真实内容（不再是调试
   * gizmo），故保留；默认模式下它仍是绿色占位矩形，继续剔除。 */
  function captureRender(options?: { includeDecals?: boolean }): string | null {
    const instance = viewer.value;
    if (!instance) return null;
    const exclude = ["effects", "spawners", "paths"];
    if (!options?.includeDecals) exclude.push("decals");
    return instance.captureScreenshot(exclude);
  }

  function resetView() {
    viewer.value?.resetView();
  }

  /** 是否有重建在飞：资产通道的部分刷新据此让位（在飞的装配自会消费
   * 新通道值，刷新反而会产生重复对象）。 */
  function isAssembling(): boolean {
    return assembling;
  }

  /**
   * 等待在飞重建**完全落地**（含尾部 GPU 预热/预编译窗口）。调度器据此
   * 把 rebuild 整程单飞串行化：编译窗口内开新一轮会 dispose 在编译的
   * 材质（three 轮询崩 + 半成品中间帧），串行后该竞态整类消失
   * （2026-10-08）。
   */
  function whenSettled(): Promise<void> {
    if (disposed || !assembling) return Promise.resolve();
    return new Promise((resolve) => {
      rebuildWaiters.push(resolve);
    });
  }

  /** 当前重建代数（token）：部分刷新跨 await 后核对，代数变化 = 新一代
   * 重建已开跑（清场过），必须中止，否则把旧对象塞回新场景。 */
  function rebuildEpoch(): number {
    return rebuildToken;
  }

  return {
    container,
    viewer,
    sceneReady,
    revision,
    ready,
    rebuild,
    unitObjects,
    applyGroupVisibility,
    applyUnitVisibility,
    applySelection,
    captureRender,
    resetView,
    setModelPickable,
    isAssembling,
    rebuildEpoch,
    whenSettled,
  };
}

export type EditorViewport = ReturnType<typeof useEditorViewport>;
