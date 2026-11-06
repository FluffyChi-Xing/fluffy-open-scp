import type * as ThreeNamespace from "three";
import { isSharedGeometry } from "@/lib/three-gltf";
import { createDepthOfFieldController } from "./viewerDepthOfField";
import { DeferredDisposalQueue } from "./deferred-disposal";
import { framePerf } from "./framePerf";
import { buildOutlineHull } from "./viewerOutline";
import {
  primeAllTextures,
  primeTexturesInBackground as primeTexturesBatched,
  type TexturePrimingHost,
} from "./viewerTexturePriming";

export interface ViewerTapHit {
  /** 命中对象；null = 点在空白处（未命中任何内容）——编辑器据此取消选中。 */
  object: ThreeNamespace.Object3D | null;
  point: ThreeNamespace.Vector3 | null;
  event: PointerEvent;
}

export interface ThreeViewerOptions {
  /** Maximum backing-store DPR; detailed editors can opt into native 2x. */
  maxPixelRatio?: number;
  /** Lower the pixel budget during camera interaction; restore on idle. */
  interactionPixelRatio?: number;
  /** 左键轻点（位移 < 4px 且未按 Shift）回调；命中空白时 object=null。 */
  onTap?: (hit: ViewerTapHit) => void;
  /** 悬停拾取（仅对 setHoverTargets 登记的目标）：进入/离开时回调。 */
  onHover?: (hit: { object: ThreeNamespace.Object3D } | null) => void;
  /** 每次真实渲染后的回调（选中框等屏幕空间 overlay 的跟随时机）。 */
  onFrame?: () => void;
}

export interface CameraPreset {
  fov: number;
  near: number;
  far: number;
  x: number;
  y: number;
  z: number;
  pitch: number;
  yaw: number;
  roll: number;
}

type Three = typeof ThreeNamespace;

// 三灯 + 环境光的基准强度；setEnvironmentBrightness 按倍率缩放这些基准值。
const KEY_LIGHT_INTENSITY = 2.2;
const FILL_LIGHT_INTENSITY = 0.5;
const RIM_LIGHT_INTENSITY = 0.65;
const AMBIENT_LIGHT_INTENSITY = 0.38;

export function disposeObject(object: ThreeNamespace.Object3D) {
  object.traverse((child) => {
    const mesh = child as ThreeNamespace.Mesh & { isLine?: boolean };
    if (mesh.isMesh || mesh.isLine) {
      // 共享缓存几何（payload 级模型缓存，见 three-gltf.getLotModelObjects）
      // 归缓存所有，清场不销毁——缓存更换时统一释放。
      if (!isSharedGeometry(mesh.geometry)) mesh.geometry?.dispose();
      const material = mesh.material;
      if (Array.isArray(material)) material.forEach((item) => item.dispose());
      else material?.dispose();
    }
  });
}

/**
 * MeshPreview 抽取出的 three.js 引擎层：轨道相机（左键旋转/中键或
 * Shift+左键平移/滚轮缩放）、三灯白模布光、Z-up 世界根组、图层组、
 * Raycaster 轻点拾取。编辑器 Gizmo（M-PE2）在此之上叠加。
 */
export class ThreeViewer {
  readonly THREE: Three;
  readonly scene: ThreeNamespace.Scene;
  readonly camera: ThreeNamespace.PerspectiveCamera;
  /** -90°X 旋转根组：RW4 的 Z-up 模型与 Unit gizmo 以游戏坐标原样加入。 */
  readonly world: ThreeNamespace.Group;
  /** 渲染画布（编辑器手柄等需要自挂 pointer 事件的组件用）。 */
  get domElement(): HTMLCanvasElement {
    return this.renderer.domElement;
  }

  /**
   * 硬件支持的最大各向异性过滤级别。建筑立面几乎总是掠射角观察，
   * three.js 默认 aniso=1 会按最大导数选 mip → 贴图发糊；引擎侧用的是
   * `tex2Dgrad` + 各向异性采样，故贴图统一按此上限设置。
   */
  get maxAnisotropy(): number {
    return this.renderer.capabilities.getMaxAnisotropy();
  }

  /** 轨道/拾取输入开关：编辑器手柄拖拽期间挂起，避免相机联动。 */
  private orbitEnabled = true;

  /** 挂起/恢复轨道相机与轻点拾取（TransformControls dragging-changed 用）。 */
  setOrbitEnabled(enabled: boolean) {
    this.orbitEnabled = enabled;
    if (!enabled) this.setHovered(null);
  }

  private readonly renderer: ThreeNamespace.WebGLRenderer;
  private readonly container: HTMLElement;
  private readonly options: ThreeViewerOptions;
  /** Y-up 场景中的内容容器（world 的父级），整体平移实现取中。 */
  private readonly content: ThreeNamespace.Group;
  private readonly keyLight: ThreeNamespace.DirectionalLight;
  private readonly fillLight: ThreeNamespace.DirectionalLight;
  private readonly rimLight: ThreeNamespace.DirectionalLight;
  private readonly ambientLight: ThreeNamespace.AmbientLight;
  private readonly raycaster: ThreeNamespace.Raycaster;
  private readonly groups = new Map<string, ThreeNamespace.Group>();
  /** 悬停拾取的目标根列表（编辑器登记 unit 图层组；空 = 不做 hover 拾取）。 */
  private hoverTargets: ThreeNamespace.Object3D[] = [];
  private lastHoverObject: ThreeNamespace.Object3D | null = null;
  private onFrameCb: (() => void) | null = null;
  private grid: ThreeNamespace.GridHelper | null = null;
  private observer: ResizeObserver | undefined;
  private frame = 0;
  private frameRadius = 5;
  /** 取景取证用（DEV 诊断）：当前地面网格的世界 Y。 */
  gridY = 0;
  /** 取景取证用（DEV 诊断）：content 归中偏移与当前取景半径。 */
  get contentPosition(): ThreeNamespace.Vector3 {
    return this.content.position;
  }
  get framingRadius(): number {
    return this.frameRadius;
  }
  private cameraDistance = 5;
  private orbitTheta = 0.34;
  private orbitPhi = 1.2;
  private readonly orbitTarget: ThreeNamespace.Vector3;
  /** 上次构图的内容中心（软跟随基准；null = 尚未构图）。 */
  private lastFrameCenter: ThreeNamespace.Vector3 | null = null;
  private readonly defaultTheta = 0.34;
  private readonly defaultPhi = 1.2;
  private selected: ThreeNamespace.Object3D | null = null;
  private pointerActive = false;
  private pointerButton = 0;
  private pointerShift = false;
  private lastX = 0;
  private lastY = 0;
  private moved = 0;
  /** 最近一次 hover 射线时间（30 Hz 节流；pointermove 在 WebView2 里可
   *  远超帧率，逐事件递归 raycast 会吃掉整个相机帧的预算）。 */
  private lastHoverAt = 0;
  /** 是否已进入相机交互质量档（拖拽超过阈值才进，避免单击闪烁）。 */
  private interactionActive = false;
  /** 容器 CSS 尺寸缓存（ResizeObserver 维护）。worldToScreen 每帧被标签/
   *  尺度标注调用多次，绝不能在投影里再读 getBoundingClientRect——那会
   *  在写完 style.left 后强制同步布局（相机拖动的主要 CPU 抖动源）。 */
  private containerWidth = 1;
  private containerHeight = 1;
  /** 按需渲染脏标记：无变化不进 GPU（编辑器形态天然低频，rebuild 期间
   * 也不再与装配争抢主线程/GPU）。任何视觉变更都必须走 invalidate()。 */
  private needsRender = true;
  /** 摄像机工作台景深(仅 cameraControlled 链路;未启用 = 直渲,零开销)。 */
  private dof: ReturnType<typeof createDepthOfFieldController>;
  private assemblingScene = false;
  private readonly deferredDisposals = new DeferredDisposalQueue(disposeObject);
  /** 上一真实出画帧的时间戳（帧间隔 → 实测 FPS）。 */
  private lastRenderAt = 0;

  setSceneAssembling(value: boolean) {
    this.assemblingScene = value;
    if (!value) {
      // 每轮装配收尾更新一次光源计数：快照里能看到"光源数 ↔ 帧成本"。
      let lights = 0;
      this.scene.traverse((object) => {
        if ((object as ThreeNamespace.Light).isLight) lights += 1;
      });
      framePerf.setLights(lights);
      this.invalidate();
    }
  }
  /** 已预热纹理（initTexture 一次性上传去重）。 */
  private readonly primedTextures = new WeakSet<ThreeNamespace.Texture>();

  invalidate() {
    this.needsRender = true;
  }

  primeTexture(texture: ThreeNamespace.Texture): void {
    if (this.primedTextures.has(texture)) return;
    this.primedTextures.add(texture);
    try {
      this.renderer.initTexture(texture);
    } catch {
      // 上传失败（上下文丢失等）→ 交给首次 draw 的常规路径
    }
  }

  /** 纹理预热宿主（场景/已预热集合/单纹理上传的显式注入面）。 */
  private textureHost(): TexturePrimingHost {
    return {
      scene: this.scene,
      isPrimed: (texture) => this.primedTextures.has(texture),
      primeTexture: (texture) => this.primeTexture(texture),
      isDisposed: () => this.disposed,
    };
  }

  /** 同步全量预热（保留给截图/测试等明确需要一次性上传的路径）。 */
  primeTextures(): void {
    primeAllTextures(this.textureHost());
  }

  /** 预算化后台预热（首帧后按 idle 切片补传，见 viewerTexturePriming）。 */
  primeTexturesInBackground(isStale: () => boolean = () => false): void {
    primeTexturesBatched(this.textureHost(), isStale);
  }

  /**
   * 场景全部材质的 GPU 程序预编译。KHR_parallel_shader_compile 可用时
   * compileAsync 异步轮询链接，老驱动回退同步。
   *
   * **竞态防御（2026-10-08 勘误）**：three 的轮询回调读取
   * `materialProperties.currentProgram`——若编译窗口内材质被 dispose
   * （新一轮重建 clearGroup / 资产刷新换对象），会抛 uncaught
   * "reading 'isReady'" 且 **promise 永不 settle**，管线（invalidate/
   * assembling 旗）随之挂死，场景冻在半成品帧（用户观测：部分建筑
   * 贴图乱码 + 控制台 isReady 报错）。故与 stale 看门狗 + 兜底超时
   * race：过期调用不再阻塞场景。
   */
  private disposed = false;
  async prepareShaders(
    isStale: () => boolean = () => false,
    objects: ThreeNamespace.Object3D = this.scene,
  ): Promise<void> {
    if (this.disposed || isStale()) return;
    const compilation = new Promise<void>((resolve) => {
      const run = () => {
        if (!this.disposed && !isStale()) try { this.renderer.compile(objects, this.camera, this.scene); } catch { /* lazy draw fallback */ }
        resolve();
      };
      const idle = (window as Window & { requestIdleCallback?: (callback: () => void, options?: { timeout: number }) => number }).requestIdleCallback;
      if (idle) idle(run, { timeout: 1000 });
      else setTimeout(run, 32);
    });
    let settled = false;
    let watchdog: ReturnType<typeof setInterval> | undefined;
    let timeout: ReturnType<typeof setTimeout> | undefined;
    try {
      await Promise.race([
        compilation,
        new Promise<void>((resolve) => {
          watchdog = setInterval(() => {
            if (settled || isStale()) resolve();
          }, 64);
        }),
        new Promise<void>((resolve) => { timeout = setTimeout(resolve, 5000); }),
      ]);
    } catch {
      // promise 层拒绝兜底（首轮 draw 走同步链接，不劣化）
    } finally {
      settled = true;
      if (watchdog) clearInterval(watchdog);
      if (timeout) clearTimeout(timeout);
    }
  }

  /** Compile completed meshes against the final lights while ground pixels bake.
   * The temporary root contains no lights and never reparents the live scene.
   */
  prepareGroupShaders(names: string[], isStale: () => boolean): Promise<void> {
    const root = new this.THREE.Group();
    for (const name of names) {
      this.groups.get(name)?.traverse((object) => {
        if ((object as ThreeNamespace.Mesh).isMesh) root.add(object.clone(false));
      });
    }
    return this.prepareShaders(isStale, root);
  }

  private hoverOutline: { target: ThreeNamespace.Object3D; hull: ThreeNamespace.Object3D } | null =
    null;
  private selectedOutline: {
    target: ThreeNamespace.Object3D;
    hull: ThreeNamespace.Object3D;
  } | null = null;

  /**
   * 悬停/选中轮廓壳（反转壳体法）：把目标网格克隆为「法线外扩 + BackSide +
   * 纯色」的壳，挂在目标对象下继承变换——轮廓贴着模型剪影走，不随视角变形
   * （2026-10-06 取代屏幕空间 AABB 方框：方框大小不定且精细灯光 bbox 爆炸）。
   * hover=虚线、selected=实线；null = 清除。
   */
  setHoverOutline(object: ThreeNamespace.Object3D | null) {
    this.applyOutline("hover", object);
  }

  setSelectedOutline(object: ThreeNamespace.Object3D | null) {
    this.applyOutline("selected", object);
  }

  private applyOutline(
    kind: "hover" | "selected",
    target: ThreeNamespace.Object3D | null,
  ) {
    const current = kind === "hover" ? this.hoverOutline : this.selectedOutline;
    if ((current?.target ?? null) === target) return;
    if (current) {
      current.hull.parent?.remove(current.hull);
      current.hull.traverse((child) => {
        const mesh = child as ThreeNamespace.Mesh;
        if (mesh.isMesh) (mesh.material as ThreeNamespace.Material).dispose();
      });
    }
    const entry = target
      ? { target, hull: buildOutlineHull(this.THREE, target, kind, this.frameRadius) }
      : null;
    if (entry && target) target.add(entry.hull);
    if (kind === "hover") this.hoverOutline = entry;
    else this.selectedOutline = entry;
    this.needsRender = true;
  }


  /** 登记悬停拾取目标（unit 图层组根）；空数组关闭 hover 拾取。 */
  setHoverTargets(targets: ThreeNamespace.Object3D[]) {
    this.hoverTargets = targets;
  }

  /** 注册/注销渲染后回调（overlay 跟随相机用；幂等覆盖）。 */
  setOnFrame(callback: (() => void) | null) {
    this.onFrameCb = callback;
  }

  /**
   * 屏幕坐标 → 地面平面交点（游戏坐标 x/y/z，z=上）。落在取景网格平面上，
   * 拖拽放置组件的落点定位用；无交点（视线平行/背向）返回 null。
   */
  groundPointAt(clientX: number, clientY: number): [number, number, number] | null {
    const rect = this.container.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    const pointer = new this.THREE.Vector2(
      ((clientX - rect.left) / rect.width) * 2 - 1,
      -((clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.raycaster.setFromCamera(pointer, this.camera);
    // 游戏地面 = world 局部 z=0 平面：场景法线 +y、过 world 原点
    // （此前误用取景网格高度 grid.position.y——内容居中后为负值，
    // 交点沿视线被推偏 = 放置落点跑偏的根因）
    const worldOrigin = new this.THREE.Vector3();
    this.world.getWorldPosition(worldOrigin);
    const plane = new this.THREE.Plane(new this.THREE.Vector3(0, 1, 0), -worldOrigin.y);
    const hit = new this.THREE.Vector3();
    if (!this.raycaster.ray.intersectPlane(plane, hit)) return null;
    const game = this.world.worldToLocal(hit.clone());
    return [game.x, game.y, Math.max(0, game.z)];
  }

  /** 世界坐标 → 容器相对屏幕像素；behind = 点在相机后方（投影无效）。 */
  worldToScreen(
    vector: ThreeNamespace.Vector3,
  ): { x: number; y: number; behind: boolean } {
    const projected = vector.clone().project(this.camera);
    return {
      x: ((projected.x + 1) / 2) * this.containerWidth,
      y: ((1 - projected.y) / 2) * this.containerHeight,
      behind: projected.z > 1 || projected.z < -1,
    };
  }

  static async create(
    container: HTMLElement,
    options: ThreeViewerOptions = {},
  ): Promise<ThreeViewer> {
    const THREE = await import("three");
    return new ThreeViewer(THREE, container, options);
  }

  private constructor(
    THREE: Three,
    container: HTMLElement,
    options: ThreeViewerOptions,
  ) {
    this.THREE = THREE;
    this.container = container;
    this.options = options;
    this.scene = new THREE.Scene();
    this.camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
    this.orbitTarget = new THREE.Vector3();
    this.renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true,
      powerPreference: "high-performance",
    });
    // 默认限制高 DPR 屏的片元成本；精细编辑器可显式提高上限。
    this.renderer.setPixelRatio(
      Math.min(window.devicePixelRatio, options.maxPixelRatio ?? 1.5),
    );
    try {
      const gl = this.renderer.getContext();
      const ext = gl.getExtension("WEBGL_debug_renderer_info");
      const label = ext
        ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)
        : gl.getParameter(gl.RENDERER);
      framePerf.setGpu(String(label ?? "unknown"));
    } catch {
      framePerf.setGpu("unknown");
    }
    container.appendChild(this.renderer.domElement);

    // 三灯白模布光：key 跟随滑杆，fill/rim 固定相对方向，保证无贴图也有立体感。
    // 基准强度供 setEnvironmentBrightness 按倍率缩放。key 用方向光（平行光）
    // ——精细模式的太阳/阴影链挂同一盏（setSunFromEnv + setShadowsEnabled）。
    this.keyLight = new THREE.DirectionalLight(0xffffff, KEY_LIGHT_INTENSITY);
    this.keyLight.castShadow = false;
    this.keyLight.shadow.mapSize.set(2048, 2048);
    this.keyLight.shadow.bias = -0.0003;
    this.keyLight.shadow.normalBias = 0.5;
    this.fillLight = new THREE.DirectionalLight(0xdde6ff, FILL_LIGHT_INTENSITY);
    this.fillLight.position.set(-1, 0.4, -0.8);
    this.rimLight = new THREE.DirectionalLight(0xffffff, RIM_LIGHT_INTENSITY);
    this.rimLight.position.set(0.4, -0.6, -1);
    this.ambientLight = new THREE.AmbientLight(0xffffff, AMBIENT_LIGHT_INTENSITY);
    this.scene.add(
      this.ambientLight,
      this.keyLight,
      this.fillLight,
      this.rimLight,
    );

    this.world = new THREE.Group();
    // RW4 为 Z-up 坐标系，-90° X 旋转后建筑在 Y-up 视图中站立。
    this.world.rotation.x = -Math.PI / 2;
    this.content = new THREE.Group();
    this.content.add(this.world);
    this.scene.add(this.content);
    this.raycaster = new THREE.Raycaster();
    this.dof = createDepthOfFieldController({
      renderer: () => this.renderer,
      scene: () => this.scene,
      camera: () => this.camera,
      getFrameRadius: () => this.frameRadius,
      isDisposed: () => this.disposed,
    });

    container.addEventListener("pointerdown", this.onPointerDown);
    container.addEventListener("pointermove", this.onPointerMove);
    container.addEventListener("pointerup", this.onPointerUp);
    container.addEventListener("pointercancel", this.onPointerUp);
    container.addEventListener("wheel", this.onWheel, { passive: false });
    container.addEventListener("contextmenu", this.onContextMenu);
    // 中键默认触发 Chromium/WebView2 自动滚动，需在 mousedown 阶段拦截。
    container.addEventListener("mousedown", this.onMouseDown);
    this.observer = new ResizeObserver(() => this.resize());
    this.observer.observe(container);
    this.resize();
    this.renderLoop();
  }

  /** 按名取图层组（懒创建），如 model/lights/decals/props/effects/spawners/paths。 */
  group(name: string): ThreeNamespace.Group {
    let group = this.groups.get(name);
    if (!group) {
      group = new this.THREE.Group();
      group.name = name;
      this.world.add(group);
      this.groups.set(name, group);
    }
    return group;
  }

  clearGroup(name: string) {
    const group = this.groups.get(name);
    if (!group) return;
    for (const child of [...group.children]) {
      group.remove(child);
      this.deferredDisposals.enqueue(child);
    }
  }

  /** 替换 model 组内容并整体取中、构图（MeshPreview 用）。 */
  setModel(objects: ThreeNamespace.Object3D[]) {
    this.clearGroup("model");
    const model = this.group("model");
    for (const object of objects) model.add(object);
    this.frameContent();
  }

  /**
   * 以 content 全部内容的包围球取中并构图：内容中心移到原点、地面网格
   * 贴底、相机 near/far 与缩放限幅按半径设定。
   *
   * 软跟随（soft）：装配是渐进的（模型/lotPlacement/草稿载荷分线异步
   * 到达），首帧之后的 bbox 材料变化若不跟随，地面网格与 content 归零
   * 就永远停在首帧的坐标系里——建筑"飘在网格上方"的观感正是这个一次性
   * latch 的产物（2026-10-27 回归根因）。软跟随把 content 绝对归零到新
   * 中心、相机与轨道目标等量反向平移（用户视角与缩放保持不动），并按新
   * 半径重贴网格；内容未变时等价于幂等重建网格，零视角扰动。near/far/
   * fov 属于用户/机位参数，软跟随不触碰。
   *
   * framingGroups：取景只统计给定图层组（如 建筑/地块/标注）。单元
   * gizmo（灯/道具/贴花/特效/生成器/路径的标记锥与垫片）是编辑器
   * chrome，其数据高度（道具锥沉到地块下数米、贴花垫片浮在上方）不应
   * 拖动地面网格与取景中心——「网格参照物」与「编辑器标记」在此分离。
   */
  frameContent(
    options: { soft?: boolean; framingGroups?: string[] } = {},
  ) {
    let box = new this.THREE.Box3().setFromObject(this.content);
    if (options.framingGroups?.length) {
      const framing = new this.THREE.Box3();
      for (const name of options.framingGroups) {
        const group = this.groups.get(name);
        if (group?.children.length) framing.expandByObject(group);
      }
      if (!framing.isEmpty()) box = framing;
    }
    if (box.isEmpty()) return;
    const sphere = box.getBoundingSphere(new this.THREE.Sphere());
    const radius = Math.max(sphere.radius, 1e-3);
    const center = box.getCenter(new this.THREE.Vector3());
    const prevCenter = this.lastFrameCenter;
    const soft = options.soft === true && prevCenter !== null;
    if (soft && prevCenter) {
      // content 从 -旧中心 平移到 -新中心，世界位移 = 旧中心 - 新中心；
      // 相机与轨道目标随世界一起平移，渲染内容在屏幕上保持不动。
      const shift = prevCenter.clone().sub(center);
      this.camera.position.add(shift);
      this.orbitTarget.add(shift);
    }
    this.lastFrameCenter = center;
    this.content.position.sub(center);
    // 取中平移后地面（Y 最小值）随内容整体下移。
    const groundY = box.min.y - center.y;
    this.scene.updateMatrixWorld(true);

    if (this.grid) {
      this.grid.geometry.dispose();
      (this.grid.material as ThreeNamespace.Material).dispose();
      this.scene.remove(this.grid);
    }
    this.grid = new this.THREE.GridHelper(radius * 12, 48, 0x8a93a6, 0x565e6e);
    const gridMaterial = this.grid.material as ThreeNamespace.Material;
    gridMaterial.transparent = true;
    gridMaterial.opacity = 0.32;
    this.grid.position.y = groundY - radius * 0.002;
    this.gridY = this.grid.position.y;
    this.scene.add(this.grid);

    this.frameRadius = radius;
    this.needsRender = true;
    if (soft) return;
    this.cameraDistance = radius * 3;
    this.camera.near = radius / 100;
    this.camera.far = radius * 40;
    this.camera.updateProjectionMatrix();
    this.resetView();
  }

  /** 回到默认视角：初始方位/俯仰、对准取中后的原点。 */
  resetView() {
    const direction = new this.THREE.Vector3(0.35, 0.4, 1).normalize();
    this.orbitPhi = Math.acos(this.THREE.MathUtils.clamp(direction.y, -1, 1));
    this.orbitTheta = Math.atan2(direction.x, direction.z);
    this.orbitTarget.set(0, 0, 0);
    this.applyCamera();
  }

  /**
   * 摄像机工作台的忠实机位：游戏 isometric 相机是「环绕机位」——
   * x/z 平移观察目标、y = 距离、pitch/yaw = 环绕角（pitch 负值 = 自上方
   * 俯视，phi = 90+pitch 自竖轴下压），相机恒看向目标，位置与朝向耦合。
   * 【2026-10-27 勘误】此前把位置与 yaw/pitch 拆成独立自由量，默认参数
   * (yaw=149) 下相机背对地块、从下缘仰视——「默认俯仰不可能从下往上看」
   * 的根因。滑杆全量程无钳制（pitch ±89 → phi 1°~179°，极端机位由用户
   * 主动选择）；fov 直接生效，near/far 按同一比例尺缩放；roll 在 lookAt
   * 之后绕视线轴叠加。仅摄像机工作台调用，其他场景的默认轨道不受影响。
   * 比例尺 scenePerGame = 2.5·radius/800：原生机位 y=800 ↔ 2.5×radius。
   */
  setCameraPreset(preset: CameraPreset) {
    const radius = Math.max(this.frameRadius, 1e-3);
    const scenePerGame = (2.5 * radius) / 800;
    this.cameraDistance = Math.max(radius * 0.05, preset.y * scenePerGame);
    this.orbitTheta = (preset.yaw * Math.PI) / 180;
    this.orbitPhi = ((90 + preset.pitch) * Math.PI) / 180;
    this.orbitTarget.set(
      preset.x * scenePerGame,
      0,
      preset.z * scenePerGame,
    );
    this.camera.fov = preset.fov;
    const near = Math.max(1e-4, preset.near * scenePerGame);
    this.camera.near = near;
    this.camera.far = Math.max(near + 1e-2, preset.far * scenePerGame);
    this.camera.updateProjectionMatrix();
    this.applyCamera();
    this.camera.rotateZ((preset.roll * Math.PI) / 180);
    this.invalidate();
  }

  /**
   * 摄像机工作台景深(仅 cameraControlled 链路;null = 关闭恢复直渲)。
   * distance 为游戏米,内部按 setCameraPreset 同款 scenePerGame 换算成
   * 场景单位;rig 懒加载,首启前到达的参数挂 pending,就绪后立即生效。
   */
  /**
   * 摄像机工作台景深(仅 cameraControlled 链路;null = 关闭恢复直渲)。
   * distance 为游戏米,controller 内部按 setCameraPreset 同款 scenePerGame
   * 换算成场景单位;rig 懒加载,首启前的参数挂 pending,就绪后立即生效。
   */
  setDepthOfField(params: { distance: number; aperture: number; softness: number } | null) {
    this.dof.setGameParams(params);
    this.invalidate();
  }

  /** key 光方位（度）；对应 MeshPreview 的方位/仰角双滑杆。 */
  /** 色调映射（精细渲染的 HDR 自发光需要；白模/示意页保持默认 NoToneMapping）。 */
  setToneMapping(mode: ThreeNamespace.ToneMapping, exposure = 1): void {
    this.renderer.toneMapping = mode;
    this.renderer.toneMappingExposure = exposure;
    this.needsRender = true;
  }

  setKeyLight(azimuthDeg: number, elevationDeg: number) {
    const radius = this.frameRadius * 2.4;
    const azimuth = (azimuthDeg * Math.PI) / 180;
    const elevation = (elevationDeg * Math.PI) / 180;
    this.keyLight.position.set(
      radius * Math.cos(elevation) * Math.sin(azimuth),
      radius * Math.sin(elevation),
      radius * Math.cos(elevation) * Math.cos(azimuth),
    );
    this.needsRender = true;
  }

  /**
   * 精细模式的 env 太阳：key 光改挂共享 env 的太阳方向/颜色（模型注入光照
   * 的场景光等价物——地面 Lambert 由它着色并获得建筑投影），shadow 相机
   * 随内容包围球取定。 null = 交回白模滑杆口径。
   */
  setSunFromEnv(
    dir: { x: number; y: number; z: number },
    color: { r: number; g: number; b: number },
  ) {
    const radius = Math.max(this.frameRadius, 1) * 2.4;
    this.keyLight.position.set(dir.x * radius, dir.y * radius, dir.z * radius);
    this.keyLight.color.setRGB(color.r, color.g, color.b);
    const extent = Math.max(this.frameRadius, 1) * 1.6;
    const camera = this.keyLight.shadow.camera;
    camera.left = -extent;
    camera.right = extent;
    camera.top = extent;
    camera.bottom = -extent;
    camera.near = 0.5;
    camera.far = radius * 3;
    camera.updateProjectionMatrix();
    this.keyLight.shadow.normalBias = Math.max(this.frameRadius, 1) * 0.01;
    this.needsRender = true;
  }

  /**
   * 阴影链开关（精细模式开）：key 光投影，内容网格全部 castShadow
   * （受影面由各 mesh 自行标 receiveShadow，如 lot 地面）。幂等——场景
   * 重建后重调只为给新网格补 castShadow 标；材质重编译仅在状态切换时。
   */
  setShadowsEnabled(enabled: boolean) {
    if (!enabled) {
      this.interactionShadowState = false;
      this.renderer.shadowMap.autoUpdate = true;
    }
    const changed = this.renderer.shadowMap.enabled !== enabled;
    this.renderer.shadowMap.enabled = enabled;
    if (enabled) this.renderer.shadowMap.autoUpdate = true;
    // PCFSoft 已被 three 弃用（内部直接转 PCF，还刷弃用告警）——直用 PCF
    this.renderer.shadowMap.type = this.THREE.PCFShadowMap;
    this.keyLight.castShadow = enabled;
    this.content.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (mesh.isMesh) mesh.castShadow = enabled && !mesh.userData.pickOnly;
    });
    if (changed) {
      this.content.traverse((child) => {
        const mesh = child as ThreeNamespace.Mesh;
        if (mesh.isMesh) {
          const materials = Array.isArray(mesh.material)
            ? mesh.material
            : [mesh.material];
          for (const material of materials) material.needsUpdate = true;
        }
      });
    }
    this.needsRender = true;
  }

  setEnvironmentBrightness(brightness: number) {
    const scale = Math.max(0, brightness);
    this.keyLight.intensity = KEY_LIGHT_INTENSITY * scale;
    this.fillLight.intensity = FILL_LIGHT_INTENSITY * scale;
    this.rimLight.intensity = RIM_LIGHT_INTENSITY * scale;
    this.ambientLight.intensity = AMBIENT_LIGHT_INTENSITY * scale;
    this.needsRender = true;
  }

  /**
   * 选中登记 + 实线轮廓壳。选中视觉 = 模型轮廓描边（反转壳体法）+ 左上角
   * name-tag（DOM 层）；旧材质染色方案已撤（对贴花/无光照材质要分别打
   * 补丁且不够醒目）。object 为 null 清除。
   */
  setSelected(object: ThreeNamespace.Object3D | null) {
    this.selected = object;
    this.applyOutline("selected", object);
    this.needsRender = true;
  }

  private applyCamera() {
    // 轨道相机：方位角/俯仰角全自由（极点略内收避免翻转），绕 orbitTarget 环绕。
    const phi = Math.min(Math.PI - 0.02, Math.max(0.02, this.orbitPhi));
    this.camera.position.setFromSpherical(
      new this.THREE.Spherical(this.cameraDistance, phi, this.orbitTheta),
    );
    this.camera.position.add(this.orbitTarget);
    this.camera.lookAt(this.orbitTarget);
    this.needsRender = true;
  }

  private orbit(dx: number, dy: number) {
    this.orbitTheta -= (dx * Math.PI) / 180 * 0.6;
    this.orbitPhi -= (dy * Math.PI) / 180 * 0.6;
    this.applyCamera();
  }

  private pan(dx: number, dy: number) {
    // 沿相机 right/up 平移轨道目标，步长随相机距离缩放。
    const scale = this.cameraDistance * 0.0016;
    const right = new this.THREE.Vector3().setFromMatrixColumn(
      this.camera.matrix,
      0,
    );
    const up = new this.THREE.Vector3().setFromMatrixColumn(this.camera.matrix, 1);
    this.orbitTarget
      .addScaledVector(right, -dx * scale)
      .addScaledVector(up, dy * scale);
    this.applyCamera();
  }

  private zoom(direction: number) {
    this.beginCameraInteraction();
    const factor = direction > 0 ? 1.12 : 1 / 1.12;
    const radius = this.frameRadius;
    this.cameraDistance = Math.min(
      radius * 12,
      Math.max(radius * 0.3, this.cameraDistance * factor),
    );
    this.applyCamera();
  }

  private onPointerDown = (event: PointerEvent) => {
    if (this.pointerActive) return;
    if (event.target !== this.renderer.domElement) return;
    if (!this.orbitEnabled) return;
    this.pointerActive = true;
    this.pointerButton = event.button;
    this.pointerShift = event.shiftKey;
    this.lastX = event.clientX;
    this.lastY = event.clientY;
    this.moved = 0;
    this.setHovered(null);
    this.container.setPointerCapture(event.pointerId);
  };

  private onPointerMove = (event: PointerEvent) => {
    if (this.pointerActive) {
      const dx = event.clientX - this.lastX;
      const dy = event.clientY - this.lastY;
      this.lastX = event.clientX;
      this.lastY = event.clientY;
      this.moved += Math.abs(dx) + Math.abs(dy);
      // 相机直接在输入事件里落地（不累积到下一帧）：orbit/pan 只是几次
      // 向量运算，提前一帧应用可消除拖拽时整帧的输入→画面延迟。
      if (this.moved > 4) this.beginCameraInteraction();
      if (this.pointerButton === 1 || this.pointerButton === 2 || (this.pointerButton === 0 && this.pointerShift)) {
        this.pan(dx, dy);
      } else if (this.pointerButton === 0) {
        this.orbit(dx, dy);
      }
      return;
    }
    if (!this.orbitEnabled || !this.options.onHover) return;
    // 30 Hz 上限：hover 只需跟手，不需要每个 pointermove 都递归射线检测。
    const now = performance.now();
    if (now - this.lastHoverAt < 33) return;
    this.lastHoverAt = now;
    this.setHovered(this.raycastTargets(event, this.hoverTargets));
  };

  private setHovered(object: ThreeNamespace.Object3D | null) {
    if (this.lastHoverObject === object) return;
    this.lastHoverObject = object;
    this.options.onHover?.(object ? { object } : null);
  }

  private raycastTargets(
    event: { clientX: number; clientY: number },
    targets: ThreeNamespace.Object3D[],
  ): ThreeNamespace.Object3D | null {
    if (!targets.length) return null;
    const rect = this.container.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    const pointer = new this.THREE.Vector2(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.raycaster.setFromCamera(pointer, this.camera);
    return this.raycaster.intersectObjects(targets, true)[0]?.object ?? null;
  }

  private onPointerUp = (event: PointerEvent) => {
    if (!this.pointerActive) return;
    this.pointerActive = false;
    // 只有真正进入过交互档（拖拽超过阈值/滚轮）才需要恢复全质量。
    if (this.interactionActive) this.scheduleQualityRestore();
    const isTap =
      this.pointerButton === 0 && !this.pointerShift && this.moved < 4;
    if (isTap) this.pick(event);
  };

  private onMouseDown = (event: MouseEvent) => {
    if (event.button === 1 || event.button === 2) event.preventDefault();
  };

  private onContextMenu = (event: MouseEvent) => {
    if (this.orbitEnabled) event.preventDefault();
  };

  private pick(event: PointerEvent) {
    const rect = this.container.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    const pointer = new this.THREE.Vector2(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.raycaster.setFromCamera(pointer, this.camera);
    const hit = this.raycaster.intersectObject(this.content, true)[0] ?? null;
    // 未命中也回调（object=null）：点空白取消选中与点地面取消同语义
    this.options.onTap?.({
      object: hit?.object ?? null,
      point: hit?.point ?? null,
      event,
    });
  }

  private onWheel = (event: WheelEvent) => {
    event.preventDefault();
    if (this.orbitEnabled) this.zoom(event.deltaY);
  };

  private resize() {
    const width = this.container.clientWidth;
    const height = this.container.clientHeight;
    if (width === 0 || height === 0) return;
    this.containerWidth = width;
    this.containerHeight = height;
    this.renderer.setSize(width, height);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.dof.setSize(width, height);
    this.needsRender = true;
  }

  private qualityRestoreTimer: ReturnType<typeof setTimeout> | null = null;
  private interactionShadowState = false;

  private beginCameraInteraction() {
    if (this.interactionActive) return;
    this.interactionActive = true;
    const limit = this.options.interactionPixelRatio;
    if (limit !== undefined) {
      const full = Math.min(window.devicePixelRatio, this.options.maxPixelRatio ?? 1.5);
      const moving = Math.min(full, limit);
      if (this.renderer.getPixelRatio() !== moving) this.renderer.setPixelRatio(moving);
    }
    if (this.renderer.shadowMap.enabled && this.keyLight.castShadow) {
      this.interactionShadowState = true;
      this.renderer.shadowMap.autoUpdate = false;
    }
    this.scheduleQualityRestore();
  }

  private scheduleQualityRestore() {
    if (this.qualityRestoreTimer !== null) clearTimeout(this.qualityRestoreTimer);
    this.qualityRestoreTimer = setTimeout(() => {
      this.qualityRestoreTimer = null;
      if (this.pointerActive) {
        this.scheduleQualityRestore();
        return;
      }
      this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, this.options.maxPixelRatio ?? 1.5));
      if (this.interactionShadowState) {
        this.renderer.shadowMap.autoUpdate = true;
        this.renderer.shadowMap.needsUpdate = true;
        this.interactionShadowState = false;
      }
      this.interactionActive = false;
      this.invalidate();
    }, 180);
  }

  private renderLoop = () => {
    this.frame = requestAnimationFrame(this.renderLoop);
    if (this.assemblingScene || !this.needsRender) return;
    this.needsRender = false;
    const now = performance.now();
    const interval = this.lastRenderAt > 0 ? now - this.lastRenderAt : 0;
    this.lastRenderAt = now;
    const started = performance.now();
    this.dof.render(() => this.renderer.render(this.scene, this.camera));
    framePerf.recordFrame(performance.now() - started, interval, {
      calls: this.renderer.info.render.calls,
      triangles: this.renderer.info.render.triangles,
      programs: this.renderer.info.programs?.length ?? 0,
      textures: this.renderer.info.memory.textures,
      geometries: this.renderer.info.memory.geometries,
      dpr: this.renderer.getPixelRatio(),
    });
    this.onFrameCb?.();
  };

  /** 视口截图；需在渲染后同一同步段读取。 */
  captureScreenshot(excludeGroups: string[] = []): string | null {
    const previousRatio = this.renderer.getPixelRatio();
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, this.options.maxPixelRatio ?? 1.5));
    const hidden: ThreeNamespace.Group[] = [];
    for (const name of excludeGroups) {
      const group = this.groups.get(name);
      if (group?.visible) {
        group.visible = false;
        hidden.push(group);
      }
    }
    try {
      this.renderer.render(this.scene, this.camera);
      return this.renderer.domElement.toDataURL("image/png");
    } catch {
      return null;
    } finally {
      for (const group of hidden) group.visible = true;
      this.renderer.setPixelRatio(previousRatio);
      this.invalidate();
    }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    if (this.qualityRestoreTimer !== null) clearTimeout(this.qualityRestoreTimer);
    cancelAnimationFrame(this.frame);
    this.observer?.disconnect();
    this.container.removeEventListener("pointerdown", this.onPointerDown);
    this.container.removeEventListener("pointermove", this.onPointerMove);
    this.container.removeEventListener("pointerup", this.onPointerUp);
    this.container.removeEventListener("pointercancel", this.onPointerUp);
    this.container.removeEventListener("wheel", this.onWheel);
    this.container.removeEventListener("mousedown", this.onMouseDown);
    this.container.removeEventListener("contextmenu", this.onContextMenu);
    const roots = [...this.scene.children];
    this.scene.clear();
    for (const root of roots) this.deferredDisposals.enqueue(root);
    this.dof.dispose();
    this.renderer.dispose();
    this.renderer.forceContextLoss();
    this.renderer.domElement.remove();
  }
}
