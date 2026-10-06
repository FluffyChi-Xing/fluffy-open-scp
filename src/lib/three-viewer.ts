import type * as ThreeNamespace from "three";
import { isSharedGeometry } from "@/lib/three-gltf";

export interface ViewerTapHit {
  /** 命中对象；null = 点在空白处（未命中任何内容）——编辑器据此取消选中。 */
  object: ThreeNamespace.Object3D | null;
  point: ThreeNamespace.Vector3 | null;
  event: PointerEvent;
}

export interface ThreeViewerOptions {
  /** 左键轻点（位移 < 4px 且未按 Shift）回调；命中空白时 object=null。 */
  onTap?: (hit: ViewerTapHit) => void;
  /** 悬停拾取（仅对 setHoverTargets 登记的目标）：进入/离开时回调。 */
  onHover?: (hit: { object: ThreeNamespace.Object3D } | null) => void;
  /** 每次真实渲染后的回调（选中框等屏幕空间 overlay 的跟随时机）。 */
  onFrame?: () => void;
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
  /** 按需渲染脏标记：无变化不进 GPU（编辑器形态天然低频，rebuild 期间
   * 也不再与装配争抢主线程/GPU）。任何视觉变更都必须走 invalidate()。 */
  private needsRender = true;

  /** 请求下一帧重绘（视觉变更后调用；相机/灯光等 viewer 内部方法已自带）。 */
  invalidate() {
    this.needsRender = true;
  }

  /** 主题色描边（品牌色 #0878FE，与 UI --brand 一致）。 */
  private static readonly OUTLINE_COLOR = 0x0878fe;

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
      ? { target, hull: this.buildOutlineHull(target, kind) }
      : null;
    if (entry) target.add(entry.hull);
    if (kind === "hover") this.hoverOutline = entry;
    else this.selectedOutline = entry;
    this.needsRender = true;
  }

  /** 轮廓壳构建：克隆目标子树的 mesh/group（跳过光源等非网格），共用几何。 */
  private buildOutlineHull(
    target: ThreeNamespace.Object3D,
    kind: "hover" | "selected",
  ): ThreeNamespace.Object3D {
    const dashed = kind === "hover";
    // 壳厚随取景半径缩放（近似恒定屏幕厚度）
    const width = Math.max(this.frameRadius, 1) * (dashed ? 0.005 : 0.01);
    const material = new this.THREE.ShaderMaterial({
      uniforms: {
        uColor: { value: new this.THREE.Color(ThreeViewer.OUTLINE_COLOR) },
        uWidth: { value: width },
        uDash: { value: dashed ? 1 : 0 },
      },
      vertexShader: `
        uniform float uWidth;
        void main() {
          vec4 mv = modelViewMatrix * vec4(position, 1.0);
          vec3 n = normalize(normalMatrix * normal);
          mv.xyz += n * uWidth;
          gl_Position = projectionMatrix * mv;
        }`,
      fragmentShader: `
        uniform vec3 uColor;
        uniform float uDash;
        void main() {
          if (uDash > 0.5 && mod(floor(gl_FragCoord.x / 9.0) + floor(gl_FragCoord.y / 9.0), 2.0) < 1.0) discard;
          gl_FragColor = vec4(uColor, 1.0);
        }`,
      side: this.THREE.BackSide,
    });
    const hull = new this.THREE.Group();
    hull.name = "__outlineHull";
    const shellFor = (mesh: ThreeNamespace.Mesh) => {
      const shell = new this.THREE.Mesh(mesh.geometry, material);
      shell.position.copy(mesh.position);
      shell.quaternion.copy(mesh.quaternion);
      shell.scale.copy(mesh.scale);
      return shell;
    };
    // 目标本身是 Mesh（锥体/占位人形等 buildUnitObject 直出）：壳挂本地
    // 恒等变换（hull 已是 target 子级，继承其世界变换）
    if ((target as ThreeNamespace.Mesh).isMesh) {
      hull.add(shellFor(target as ThreeNamespace.Mesh));
      return hull;
    }
    const walk = (src: ThreeNamespace.Object3D, dst: ThreeNamespace.Object3D) => {
      for (const child of [...src.children]) {
        const mesh = child as ThreeNamespace.Mesh;
        if (mesh.isMesh) {
          dst.add(shellFor(mesh));
        } else if ((child as ThreeNamespace.Object3D).isGroup) {
          const group = new this.THREE.Group();
          group.position.copy(child.position);
          group.quaternion.copy(child.quaternion);
          group.scale.copy(child.scale);
          dst.add(group);
          walk(child, group);
        }
        // 光源/线条等其他类型跳过（路径线无轮廓语义）
      }
    };
    walk(target, hull);
    return hull;
  }

  /** 登记悬停拾取目标（unit 图层组根）；空数组关闭 hover 拾取。 */
  setHoverTargets(targets: ThreeNamespace.Object3D[]) {
    this.hoverTargets = targets;
  }

  /** 注册/注销渲染后回调（overlay 跟随相机用；幂等覆盖）。 */
  setOnFrame(callback: (() => void) | null) {
    this.onFrameCb = callback;
  }

  /** 世界坐标 → 容器相对屏幕像素；behind = 点在相机后方（投影无效）。 */
  worldToScreen(
    vector: ThreeNamespace.Vector3,
  ): { x: number; y: number; behind: boolean } {
    const projected = vector.clone().project(this.camera);
    const rect = this.container.getBoundingClientRect();
    return {
      x: ((projected.x + 1) / 2) * rect.width,
      y: ((1 - projected.y) / 2) * rect.height,
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
    // 高 DPR 屏（125%/150% 缩放）全屏片元成本翻倍，钳制 1.5 保帧率
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
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
    this.needsRender = true;
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
    const changed = this.renderer.shadowMap.enabled !== enabled;
    this.renderer.shadowMap.enabled = enabled;
    this.renderer.shadowMap.type = this.THREE.PCFSoftShadowMap;
    this.keyLight.castShadow = enabled;
    this.content.traverse((child) => {
      const mesh = child as ThreeNamespace.Mesh;
      if (mesh.isMesh) mesh.castShadow = enabled;
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

  /** 环境亮度倍率（日/夜模拟）：0 ≈ 夜、1 = 默认观感、2 ≈ 正午。 */
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
    // 拖拽期间悬停框无意义，清掉
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
      if (this.pointerButton === 1 || (this.pointerButton === 0 && this.pointerShift)) {
        this.pan(dx, dy);
      } else if (this.pointerButton === 0) {
        this.orbit(dx, dy);
      }
      return;
    }
    // 非拖拽悬停：仅对登记目标做轻量拾取（unit 图层组，不含建筑/地面）
    if (!this.orbitEnabled || !this.options.onHover) return;
    this.setHovered(this.raycastTargets(event, this.hoverTargets));
  };

  private setHovered(object: ThreeNamespace.Object3D | null) {
    if (this.lastHoverObject === object) return;
    this.lastHoverObject = object;
    this.options.onHover?.(object ? { object } : null);
  }

  /** 对目标根列表做射线拾取（回收里向上找 unitId 由使用方负责）。 */
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
    this.renderer.setSize(width, height);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.needsRender = true;
  }

  private renderLoop = () => {
    this.frame = requestAnimationFrame(this.renderLoop);
    if (!this.needsRender) return;
    this.needsRender = false;
    this.renderer.render(this.scene, this.camera);
    // 渲染后回调：屏幕空间 overlay（选中/悬停描边框）在此跟随相机
    this.onFrameCb?.();
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
