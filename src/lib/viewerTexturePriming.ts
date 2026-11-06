import type * as ThreeNamespace from "three";

/** 纹理预热所需的最小视口接口（与 ThreeViewer 解耦，便于测试）。 */
export interface TexturePrimingHost {
  scene: ThreeNamespace.Scene;
  /** 已预热的纹理（去重；命中则跳过收集）。 */
  isPrimed(texture: ThreeNamespace.Texture): boolean;
  /** 单纹理 initTexture。 */
  primeTexture(texture: ThreeNamespace.Texture): void;
  isDisposed(): boolean;
}

/** 收集场景材质里尚未预热的纹理（去重）。 */
export function collectSceneTextures(
  host: TexturePrimingHost,
): ThreeNamespace.Texture[] {
  const textures: ThreeNamespace.Texture[] = [];
  const seen = new Set<ThreeNamespace.Texture>();
  host.scene.traverse((object) => {
    const mesh = object as ThreeNamespace.Mesh;
    if (!mesh.isMesh && !(object as ThreeNamespace.Sprite).isSprite) return;
    const material = mesh.material as
      | ThreeNamespace.Material
      | ThreeNamespace.Material[]
      | undefined;
    if (!material) return;
    const list = Array.isArray(material) ? material : [material];
    for (const entry of list) {
      for (const value of Object.values(entry as unknown as Record<string, unknown>)) {
        const texture = value as ThreeNamespace.Texture;
        if (!texture?.isTexture || seen.has(texture) || host.isPrimed(texture)) {
          continue;
        }
        seen.add(texture);
        textures.push(texture);
      }
    }
  });
  return textures;
}

/** 同步全量预热（截图/测试等明确需要一次性上传的路径）。 */
export function primeAllTextures(host: TexturePrimingHost): void {
  for (const texture of collectSceneTextures(host)) host.primeTexture(texture);
}

/**
 * 预算化后台预热：每片最多 ~6ms 上传，避免整场 `initTexture` 在首帧后
 * 主线程上一次性跑完（大图集上传可达数百 ms = 视口冻帧）。
 * `isStale` 返回 true（新一代重建已清场）时立即放弃剩余队列。
 */
export function primeTexturesInBackground(
  host: TexturePrimingHost,
  isStale: () => boolean = () => false,
): void {
  const pending = collectSceneTextures(host);
  if (!pending.length) return;
  const schedule = (run: () => void) => {
    const idle = (window as Window & {
      requestIdleCallback?: (
        callback: () => void,
        options?: { timeout: number },
      ) => number;
    }).requestIdleCallback;
    if (idle) idle(run, { timeout: 500 });
    else setTimeout(run, 0);
  };
  const runChunk = () => {
    if (host.isDisposed() || isStale()) return;
    const started = performance.now();
    do {
      const texture = pending.shift();
      if (!texture) return;
      host.primeTexture(texture);
    } while (pending.length && performance.now() - started < 6);
    if (pending.length) schedule(runChunk);
  };
  schedule(runChunk);
}
