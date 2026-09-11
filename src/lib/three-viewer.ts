import type * as ThreeNamespace from "three";

export interface ViewerTapHit {
  object: ThreeNamespace.Object3D;
  point: ThreeNamespace.Vector3;
  event: PointerEvent;
}

export interface ThreeViewerOptions {
  /** 左键轻点（位移 < 4px 且未按 Shift）命中 content 内容时回调。 */
  onTap?: (hit: ViewerTapHit) => void;
}

type Three = typeof ThreeNamespace;

// 三灯 + 环境光的基准强度；setEnvironmentBrightness 按倍率缩放这些基准值。
const KEY_LIGHT_INTENSITY = 2.2;
const FILL_LIGHT_INTENSITY = 0.5;
const RIM_LIGHT_INTENSITY = 0.65;
const AMBIENT_LIGHT_INTENSITY = 0.38;

export function disposeObject(object: ThreeNamespace.Object3D) {
  object.traverse((child) => {
    const mesh = child as ThreeNamespace.Mesh;
    if (mesh.isMesh) {
      mesh.geometry?.dispose();
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

  /** 轨道/拾取输入开关：编辑器手柄拖拽期间挂起，避免相机联动。 */
  private orbitEnabled = true;

  /** 挂起/恢复轨道相机与轻点拾取（TransformControls dragging-changed 用）。 */
  setOrbitEnabled(enabled: boolean) {
    this.orbitEnabled = enabled;
  }

  private readonly renderer: ThreeNamespace.WebGLRenderer;
  private readonly container: HTMLElement;
  private readonly options: ThreeViewerOptions;
  /** Y-up 场景中的内容容器（world 的父级），整体平移实现取中。 */
  private readonly content: ThreeNamespace.Group;
  private readonly keyLight: ThreeNamespace.PointLight;
  private readonly fillLight: ThreeNamespace.DirectionalLight;
  private readonly rimLight: ThreeNamespace.DirectionalLight;
  private readonly ambientLight: ThreeNamespace.AmbientLight;
  private readonly raycaster: ThreeNamespace.Raycaster;
  private readonly groups = new Map<string, ThreeNamespace.Group>();
  private grid: ThreeNamespace.GridHelper | null = null;
  private observer: ResizeObserver | undefined;
  private frame = 0;
  private frameRadius = 5;
  private cameraDistance = 5;
  private orbitTheta = 0.34;
  private orbitPhi = 1.2;
  private readonly orbitTarget: ThreeNamespace.Vector3;
  private readonly defaultTheta = 0.34;
  private readonly defaultPhi = 1.2;
  private selected: ThreeNamespace.Object3D | null = null;
  private pointerActive = false;
  private pointerButton = 0;
  private pointerShift = false;
  private lastX = 0;
  private lastY = 0;
  private moved = 0;

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
    // 高 DPR 屏（125%/150% 缩放）全屏片元成本翻倍，钳制 1.5 保帧率
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
    container.appendChild(this.renderer.domElement);

    // 三灯白模布光：key 跟随滑杆，fill/rim 固定相对方向，保证无贴图也有立体感。
    // 基准强度供 setEnvironmentBrightness 按倍率缩放。
    this.keyLight = new THREE.PointLight(0xffffff, KEY_LIGHT_INTENSITY, 0, 0);
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

    container.addEventListener("pointerdown", this.onPointerDown);
    container.addEventListener("pointermove", this.onPointerMove);
    container.addEventListener("pointerup", this.onPointerUp);
    container.addEventListener("pointercancel", this.onPointerUp);
    container.addEventListener("wheel", this.onWheel, { passive: false });
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
      disposeObject(child);
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
   */
  frameContent() {
    const box = new this.THREE.Box3().setFromObject(this.content);
    if (box.isEmpty()) return;
    const sphere = box.getBoundingSphere(new this.THREE.Sphere());
    const radius = Math.max(sphere.radius, 1e-3);
    const center = box.getCenter(new this.THREE.Vector3());
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
    this.scene.add(this.grid);

    this.frameRadius = radius;
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

  /** key 光方位（度）；对应 MeshPreview 的方位/仰角双滑杆。 */
  setKeyLight(azimuthDeg: number, elevationDeg: number) {
    const radius = this.frameRadius * 2.4;
    const azimuth = (azimuthDeg * Math.PI) / 180;
    const elevation = (elevationDeg * Math.PI) / 180;
    this.keyLight.position.set(
      radius * Math.cos(elevation) * Math.sin(azimuth),
      radius * Math.sin(elevation),
      radius * Math.cos(elevation) * Math.cos(azimuth),
    );
  }

  /** 环境亮度倍率（日/夜模拟）：0 ≈ 夜、1 = 默认观感、2 ≈ 正午。 */
  setEnvironmentBrightness(brightness: number) {
    const scale = Math.max(0, brightness);
    this.keyLight.intensity = KEY_LIGHT_INTENSITY * scale;
    this.fillLight.intensity = FILL_LIGHT_INTENSITY * scale;
    this.rimLight.intensity = RIM_LIGHT_INTENSITY * scale;
    this.ambientLight.intensity = AMBIENT_LIGHT_INTENSITY * scale;
  }

  /** 选中高亮（emissive），object 为 null 清除。 */
  setSelected(object: ThreeNamespace.Object3D | null) {
    if (this.selected === object) return;
    this.applyHighlight(this.selected, false);
    this.selected = object;
    this.applyHighlight(this.selected, true);
  }

  private applyHighlight(object: ThreeNamespace.Object3D | null, on: boolean) {
    object?.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (!mesh.isMesh) return;
      const materials = Array.isArray(mesh.material)
        ? mesh.material
        : [mesh.material];
      for (const material of materials) {
        const standard = material as ThreeNamespace.MeshStandardMaterial;
        standard.emissive?.setHex(on ? 0x2f5fb0 : 0x000000);
      }
    });
  }

  private applyCamera() {
    // 轨道相机：方位角/俯仰角全自由（极点略内收避免翻转），绕 orbitTarget 环绕。
    const phi = Math.min(Math.PI - 0.02, Math.max(0.02, this.orbitPhi));
    this.camera.position.setFromSpherical(
      new this.THREE.Spherical(this.cameraDistance, phi, this.orbitTheta),
    );
    this.camera.position.add(this.orbitTarget);
    this.camera.lookAt(this.orbitTarget);
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
    if (!this.orbitEnabled) return;
    this.pointerActive = true;
    this.pointerButton = event.button;
    this.pointerShift = event.shiftKey;
    this.lastX = event.clientX;
    this.lastY = event.clientY;
    this.moved = 0;
    this.container.setPointerCapture(event.pointerId);
  };

  private onPointerMove = (event: PointerEvent) => {
    if (!this.pointerActive) return;
    const dx = event.clientX - this.lastX;
    const dy = event.clientY - this.lastY;
    this.lastX = event.clientX;
    this.lastY = event.clientY;
    this.moved += Math.abs(dx) + Math.abs(dy);
    if (this.pointerButton === 1 || (this.pointerButton === 0 && this.pointerShift)) {
      this.pan(dx, dy);
    } else if (this.pointerButton === 0) {
      this.orbit(dx, dy);
    }
  };

  private onPointerUp = (event: PointerEvent) => {
    if (!this.pointerActive) return;
    this.pointerActive = false;
    const isTap =
      this.pointerButton === 0 && !this.pointerShift && this.moved < 4;
    if (isTap) this.pick(event);
  };

  private onMouseDown = (event: MouseEvent) => {
    if (event.button === 1) event.preventDefault();
  };

  private pick(event: PointerEvent) {
    const rect = this.container.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    const pointer = new this.THREE.Vector2(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.raycaster.setFromCamera(pointer, this.camera);
    const hits = this.raycaster.intersectObject(this.content, true);
    const hit = hits[0];
    if (hit && this.options.onTap) {
      this.options.onTap({ object: hit.object, point: hit.point, event });
    }
  }

  private onWheel = (event: WheelEvent) => {
    event.preventDefault();
    if (this.orbitEnabled) this.zoom(event.deltaY);
  };

  private resize() {
    const width = this.container.clientWidth;
    const height = this.container.clientHeight;
    if (width === 0 || height === 0) return;
    this.renderer.setSize(width, height);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
  }

  private renderLoop = () => {
    this.frame = requestAnimationFrame(this.renderLoop);
    this.renderer.render(this.scene, this.camera);
  };

  /**
   * 视口截图：隐藏指定图层组 → 立即渲染 → canvas.toDataURL（PNG）→
   * 恢复可见性。需在渲染后同一同步段读取（未开 preserveDrawingBuffer）。
   */
  captureScreenshot(excludeGroups: string[] = []): string | null {
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
    }
  }

  dispose() {
    cancelAnimationFrame(this.frame);
    this.observer?.disconnect();
    this.container.removeEventListener("pointerdown", this.onPointerDown);
    this.container.removeEventListener("pointermove", this.onPointerMove);
    this.container.removeEventListener("pointerup", this.onPointerUp);
    this.container.removeEventListener("pointercancel", this.onPointerUp);
    this.container.removeEventListener("wheel", this.onWheel);
    this.container.removeEventListener("mousedown", this.onMouseDown);
    disposeObject(this.scene);
    this.grid?.geometry.dispose();
    (this.grid?.material as ThreeNamespace.Material | null)?.dispose();
    this.keyLight.dispose();
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }
}
