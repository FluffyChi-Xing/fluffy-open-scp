import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { collectNodeTransfers, extractObjectTree, type GltfNodeDto, type GltfParseRequest, type GltfParseResponse } from "./gltfParseDto";

/**
 * GLB 解析 Worker 入口（Vite module worker）。
 *
 * PE 载荷的 GLB 由自家 Rust 导出器产出：零内嵌纹理、无骨架/动画——
 * GLTFLoader 在 worker 内解析无 DOM/纹理依赖。提取为扁平节点 DTO 后
 * transferable 转移回主线程重建（主线程只付 memcpy 级 BufferAttribute
 * 包装，不再承担数百 ms 的同步 GLB 解析）。
 */

interface WorkerScope {
  postMessage(
    message: GltfParseResponse,
    transfers?: ArrayBuffer[],
  ): void;
  onmessage: ((event: MessageEvent<GltfParseRequest>) => void) | null;
}

export async function handleGltfParse(
  request: GltfParseRequest,
): Promise<GltfNodeDto[]> {
  const loader = new GLTFLoader();
  const sceneNodes: GltfNodeDto[] = [];
  for (const glb of request.glbs) {
    const gltf = await loader.parseAsync(glb, "");
    // 根旋转剥离（gltf.rs 根节点自带 Z-up→Y-up 的 -90°X，视口 world 组
    // 已做同款旋转）——与主线程旧路径 `child.rotation.set(0,0,0)` 等价。
    for (const child of gltf.scene.children) child.rotation.set(0, 0, 0);
    // gltf.scene 自身作为根节点纳入（每 GLB 恰一个根，与旧路径同构）
    sceneNodes.push(...extractObjectTree(gltf.scene, THREE));
  }
  return sceneNodes;
}

export function runGltfParseWorker(scope: WorkerScope): void {
  scope.onmessage = (event: MessageEvent<GltfParseRequest>) => {
    const { id, cloneResponse } = event.data;
    void handleGltfParse(event.data)
      .then((nodes) => {
        // cloneResponse = 结构化克隆应答（EP1 乱码二分：排除 transfer/
        // detach 边界；拷贝成本仅取证期间存在）
        scope.postMessage(
          { id, nodes },
          cloneResponse
            ? undefined
            : (collectNodeTransfers(nodes) as ArrayBuffer[]),
        );
      })
      .catch((error: unknown) => {
        scope.postMessage({
          id,
          error: error instanceof Error ? error.message : String(error),
        });
      });
  };
}

/* Vite module worker 全局入口。 */
runGltfParseWorker(self as unknown as WorkerScope);
