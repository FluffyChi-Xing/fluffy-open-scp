import { useGamePackagesStore } from "@/stores/gamePackages";

/**
 * 资源浏览器状态入口。实现位于 Pinia store（stores/gamePackages.ts），
 * 页面切换后保留打开的包，页面负责管理搜索词的生命周期。
 */
export function useGamePackages() {
  return useGamePackagesStore();
}
