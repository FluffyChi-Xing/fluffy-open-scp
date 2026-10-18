/** 程序化生成的确定性随机源：mulberry32 PRNG + 坐标散列。
 *
 * 参考项目的 rng.ts 用 Blender 位级哈希是为了和 .blend 节点图对拍；
 * 这里的移植只需要「同参数 + 同 seed → 同建筑」的性质，取轻量实现。
 */

/** mulberry32：32 位状态 PRNG，返回 [0, 1)。 */
export function mulberry32(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** 2D 坐标散列 → [0, 1)：同 (x, y, seed) 稳定，无需顺序重放。 */
export function hash2(x: number, y: number, seed: number): number {
  let h = (seed ^ 0x9e3779b9) >>> 0;
  h = Math.imul(h ^ (x | 0), 0x85ebca6b) >>> 0;
  h ^= h >>> 13;
  h = Math.imul(h ^ (y | 0), 0xc2b2ae35) >>> 0;
  h ^= h >>> 16;
  return h / 4294967296;
}

export function pick<T>(rng: () => number, items: readonly T[]): T {
  return items[Math.min(items.length - 1, Math.floor(rng() * items.length))];
}
