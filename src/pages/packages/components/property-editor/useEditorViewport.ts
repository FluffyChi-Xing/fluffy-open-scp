import { onBeforeUnmount, onMounted, ref, shallowRef } from "vue";
import { ThreeViewer } from "@/lib/three-viewer";
import type * as ThreeNamespace from "three";

/** 场景图层组注册表：底座只认组名，组的业务语义由装配层定义。 */
export const VIEWPORT_GROUPS = [
  "model",
  "lot",
  "lights",
  "props",
  "decals",
  "effects",
  "spawners",
  "paths",
] as const;
export type ViewportGroupName = (typeof VIEWPORT_GROUPS)[number];

/**
 * 单次 rebuild 的装配上下文：装配层（scene contributor）在回调内完成
 * 业务场景搭建，底座负责前后清理、防竞态与收尾。
 */
export interface EditorViewportRebuildCtx {
  viewer: ThreeViewer;
  THREE: typeof ThreeNamespace;
  token: number;
  /** 异步装配过程中重建是否已被新一代取代（取代后应放弃并回收资源）。 */
  isStale(): boolean;
  /** 登记本轮创建的贴图 blob URL，rebuild 时统一回收。 */
  registerTextureUrl(url: string): void;
  /** unitId → Object3D 拾取/可见性/选中注册表（装配层写入）。 */
  unitObjects: Map<string, ThreeNamespace.Object3D>;
}

/**
 * 编辑器视口底座：viewer 生命周期、拾取→select、rebuild 骨架（清组/
 * 防竞态/贴图 URL 回收/收尾构图）、可见性与选中应用、渲染截图。
 * 与使用形态无关（预览/property 编辑/rw4 编辑共用），业务场景装配通过
 * rebuild(assembly) 回调注入。
 */
export function useEditorViewport(options: {
  onTapUnit: (id: string | null) => void;
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
  let readyResolve: (() => void) | null = null;
  /** viewer 创建完成信号（装配层 onMounted 后据此触发首次 rebuild）。 */
  const ready = new Promise<void>((resolve) => {
    readyResolve = resolve;
  });

  onMounted(async () => {
    const element = container.value;
    if (!element) return;
    viewer.value = await ThreeViewer.create(element, {
      onTap: (hit) => {
        let node: ThreeNamespace.Object3D | null = hit.object;
        while (node) {
          if (typeof node.userData?.unitId === "string") {
            options.onTapUnit(node.userData.unitId);
            return;
          }
          node = node.parent;
        }
        options.onTapUnit(null);
      },
    });
    readyResolve?.();
  });
  onBeforeUnmount(() => {
    rebuildToken += 1;
    viewer.value?.dispose();
    viewer.value = null;
    unitObjects.clear();
    for (const url of textureUrls) URL.revokeObjectURL(url);
    textureUrls.length = 0;
  });

  /**
   * 场景真实光源总数上限：WebGL 前向渲染每个片元都评估全部光源，
   * 多灯地块（路灯密集的建筑群）推近镜头时片元数×光源数导致掉帧。
   * 超限时从整体强度最弱的单元开始摘除真实光源（保留透明拾取代理）。
   */
  const MAX_REAL_LIGHTS = 24;

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
    options: { reframe?: boolean } = {},
  ) {
    const instance = viewer.value;
    if (!instance) return;
    const token = ++rebuildToken;
    for (const name of VIEWPORT_GROUPS) instance.clearGroup(name);
    unitObjects.clear();
    for (const url of textureUrls) URL.revokeObjectURL(url);
    textureUrls.length = 0;
    const ctx: EditorViewportRebuildCtx = {
      viewer: instance,
      THREE: instance.THREE,
      token,
      isStale: () => token !== rebuildToken,
      registerTextureUrl: (url) => textureUrls.push(url),
      unitObjects,
    };
    await assembly(ctx);
    if (token !== rebuildToken) return;
    pruneExcessLights();
    if (options.reframe || !framed) {
      instance.frameContent();
      framed = true;
    }
    sceneReady.value = true;
    revision.value += 1;
  }

  function applyGroupVisibility(map: Record<string, boolean>) {
    const instance = viewer.value;
    if (!instance) return;
    for (const name of VIEWPORT_GROUPS) {
      instance.group(name).visible = map[name] !== false;
    }
  }

  function applyUnitVisibility(hidden: Set<string>) {
    for (const [id, object] of unitObjects) {
      object.visible = !hidden.has(id);
    }
  }

  function applySelection(selectedId: string | null) {
    const selected = selectedId ? unitObjects.get(selectedId) ?? null : null;
    viewer.value?.setSelected(selected);
  }

  /** 渲染图导出：默认剔除 props/decals/spawners/effects/paths 等 gizmo 组
   *（保留模型与灯光），返回 PNG dataURL；null = 截图失败。 */
  function captureRender(): string | null {
    const instance = viewer.value;
    if (!instance) return null;
    const exclude = ["props", "decals", "effects", "spawners", "paths"];
    return instance.captureScreenshot(exclude);
  }

  function resetView() {
    viewer.value?.resetView();
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
  };
}

export type EditorViewport = ReturnType<typeof useEditorViewport>;
