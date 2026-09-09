import { useGamePackagesStore } from "@/stores/gamePackages";

/**
 * 资源浏览器状态入口。实现位于 Pinia store（stores/gamePackages.ts），
 * 页面切换后打开的 package / 页码 / 筛选 / 预览全部保留。
 */
export function useGamePackages() {
  return useGamePackagesStore();
}
