import type { Pixels } from "./groundCompose";

/** Precompute separable wrapping/weights once per output axis, keeping IEEE operations unchanged. */
export function createGroundPatternSampler(
  outWidth: number,
  outHeight: number,
  tilesX: number,
  tilesY: number,
) {
  const axes = new Map<
    string,
    { first: Int32Array; second: Int32Array; weight: Float64Array }
  >();
  function axis(count: number, extent: number, tiles: number, flip: boolean) {
    const key = [count, extent, tiles, flip].join(":");
    const cached = axes.get(key);
    if (cached) return cached;
    const first = new Int32Array(count),
      second = new Int32Array(count),
      weight = new Float64Array(count);
    const wrap = (v: number) =>
      v < 0 ? v + extent : v >= extent ? v - extent : v;
    for (let i = 0; i < count; i++) {
      const uv = (i + 0.5) / count;
      const repeat = (flip ? 0.5 - uv : uv - 0.5) * tiles;
      const f = (repeat - Math.floor(repeat)) * extent - 0.5;
      const start = Math.floor(f);
      first[i] = wrap(start);
      second[i] = wrap(start + 1);
      weight[i] = f - start;
    }
    const result = { first, second, weight };
    axes.set(key, result);
    return result;
  }
  const sources = new WeakMap<
    Pixels<ArrayBufferLike>,
    (x: number, y: number) => [number, number, number]
  >();
  function prepare(source: Pixels<ArrayBufferLike>) {
    const horizontal = axis(outWidth, source.width, tilesX, false);
    const vertical = axis(outHeight, source.height, tilesY, true);
    const output: [number, number, number] = [0, 0, 0];
    return (x: number, y: number): [number, number, number] => {
      const topLeft =
        (vertical.first[y] * source.width + horizontal.first[x]) * 4;
      const topRight =
        (vertical.first[y] * source.width + horizontal.second[x]) * 4;
      const bottomLeft =
        (vertical.second[y] * source.width + horizontal.first[x]) * 4;
      const bottomRight =
        (vertical.second[y] * source.width + horizontal.second[x]) * 4;
      const tx = horizontal.weight[x],
        ty = vertical.weight[y],
        data = source.data;
      for (let c = 0; c < 3; c++) {
        const top =
          data[topLeft + c] + (data[topRight + c] - data[topLeft + c]) * tx;
        const bottom =
          data[bottomLeft + c] +
          (data[bottomRight + c] - data[bottomLeft + c]) * tx;
        output[c] = top + (bottom - top) * ty;
      }
      return output;
    };
  }
  return (
    source: Pixels<ArrayBufferLike>,
    x: number,
    y: number,
  ): [number, number, number] => {
    let sampler = sources.get(source);
    if (!sampler) {
      sampler = prepare(source);
      sources.set(source, sampler);
    }
    return sampler(x, y);
  };
}
