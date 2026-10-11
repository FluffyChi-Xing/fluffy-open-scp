export type NoiseAlgorithm = "perlin" | "value";

export interface MapGenerationConfig {
  width: number;
  height: number;
  seed: number;
  algorithm: NoiseAlgorithm;
  octaves: number;
  frequency: number;
  persistence: number;
  grassBrush: number;
  treeBrush: number;
}

function hash(x: number, y: number, seed: number): number {
  let n = Math.imul(x + seed * 374761393, 668265263) ^ Math.imul(y, 1442695041);
  n = Math.imul(n ^ (n >>> 13), 1274126177);
  return ((n ^ (n >>> 16)) >>> 0) / 4294967295;
}
function smooth(t: number): number {
  return t * t * (3 - 2 * t);
}
function perlinAt(x: number, y: number, seed: number): number {
  const ix = Math.floor(x),
    iy = Math.floor(y),
    dx = x - ix,
    dy = y - iy;
  const fade = (v: number) => v * v * v * (v * (v * 6 - 15) + 10);
  const dot = (a: number, b: number) => {
    const angle = hash(a, b, seed) * Math.PI * 2;
    return Math.cos(angle) * (x - a) + Math.sin(angle) * (y - b);
  };
  const u = fade(dx),
    v = fade(dy);
  const a = dot(ix, iy),
    b = dot(ix + 1, iy),
    c = dot(ix, iy + 1),
    d = dot(ix + 1, iy + 1);
  return 0.5 + (a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v) * 0.5;
}
function valueAt(x: number, y: number, seed: number): number {
  const x0 = Math.floor(x),
    y0 = Math.floor(y);
  const tx = smooth(x - x0),
    ty = smooth(y - y0);
  const a = hash(x0, y0, seed),
    b = hash(x0 + 1, y0, seed);
  const c = hash(x0, y0 + 1, seed),
    d = hash(x0 + 1, y0 + 1, seed);
  return a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty;
}

export function generateHeightField(config: MapGenerationConfig): Float32Array {
  const width = Math.max(2, Math.min(512, Math.floor(config.width) || 128));
  const height = Math.max(2, Math.min(512, Math.floor(config.height) || 128));
  const out = new Float32Array(width * height);
  const octaves = Math.max(1, Math.min(12, Math.floor(config.octaves) || 1));
  let min = Infinity,
    max = -Infinity;
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      let amplitude = 1,
        frequency = Math.max(0.001, config.frequency),
        total = 0,
        weight = 0;
      for (let o = 0; o < octaves; o++) {
        const n =
          config.algorithm === "value"
            ? valueAt(
                (x * frequency) / width,
                (y * frequency) / height,
                config.seed + o * 101,
              )
            : perlinAt(
                (x * frequency) / width,
                (y * frequency) / height,
                config.seed + o * 101,
              );
        total += n * amplitude;
        weight += amplitude;
        amplitude *= Math.max(0, Math.min(1, config.persistence || 0));
        frequency *= 2;
      }
      const v = total / weight;
      out[y * width + x] = v;
      min = Math.min(min, out[y * width + x]!);
      max = Math.max(max, out[y * width + x]!);
    }
  const span = max - min || 1;
  for (let i = 0; i < out.length; i++) out[i] = (out[i] - min) / span;
  return out;
}

export function buildVegetationMasks(
  field: Float32Array,
  width: number,
  height: number,
  config: MapGenerationConfig,
) {
  const grass = new Uint8Array(field.length),
    trees = new Uint8Array(field.length);
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      const left = field[y * width + Math.max(0, x - 1)],
        right = field[y * width + Math.min(width - 1, x + 1)];
      const up = field[Math.max(0, y - 1) * width + x],
        down = field[Math.min(height - 1, y + 1) * width + x];
      const slope = Math.abs(right - left) + Math.abs(down - up);
      grass[i] = Math.round(
        Math.max(0, Math.min(1, config.grassBrush * (1 - slope * 2))) * 255,
      );
      trees[i] = Math.round(
        Math.max(
          0,
          Math.min(
            1,
            config.treeBrush * (1 - slope * 3) * (0.35 + field[i] * 0.65),
          ),
        ) * 255,
      );
    }
  return { grass, trees };
}

export function previewSvg(
  field: Float32Array,
  width: number,
  height: number,
  masks?: ReturnType<typeof buildVegetationMasks>,
): string {
  const cells: string[] = [];
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const i = y * width + x,
        v = Math.round(field[i] * 255),
        g = masks?.grass[i] ?? 0,
        t = masks?.trees[i] ?? 0;
      const color = `rgb(${v},${Math.min(255, v + g / 4)},${Math.min(255, v + t / 3)})`;
      cells.push(
        `<rect x="${x}" y="${y}" width="1" height="1" fill="${color}"/>`,
      );
    }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" shape-rendering="crispEdges">${cells.join("")}</svg>`;
}
