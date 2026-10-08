import * as THREE from "three";
import type { Region3DData } from "@/lib/region-map";

/** Raster-derived road preview, not decoded EcoPathSet geometry.
 * No inferred bridge piers: bridge classification requires PathEntry data.
 */
export function createRoadPreview(data: Region3DData, mask: Uint8Array,
  heightWorld: (x: number, y: number) => number): THREE.Group {
  const group = new THREE.Group();
  group.name = "roads";
  const S = data.size, SPAN = S * data.metersPerPixel, ORG = data.originWorld;
  const rp: number[] = [];
  const rc: number[] = [];
  const ri: number[] = [];
  const uv: number[] = [];
  const cells = S; // 16m/格
  const cellM = SPAN / cells;
  const isRoad = (x: number, y: number): boolean =>
    x >= 0 && y >= 0 && x < cells && y < cells && mask[y * S + x] === 1;
  // 1) 链提取（8 邻接走带，端点优先、方向延续；环路/交叉口由兜底遍历收尾）
  const DIRS8: [number, number][] = [
    [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1],
  ];
  const visited = new Uint8Array(cells * cells);
  const nbrs = (x: number, y: number): [number, number][] => {
    const out: [number, number][] = [];
    for (const [dx, dy] of DIRS8) {
      const nx = x + dx;
      const ny = y + dy;
      if (isRoad(nx, ny) && !visited[ny * cells + nx]) out.push([nx, ny]);
    }
    return out;
  };
  const walkChain = (sx: number, sy: number): [number, number][] => {
    const chain: [number, number][] = [[sx, sy]];
    visited[sy * cells + sx] = 1;
    for (;;) {
      const [cx, cy] = chain[chain.length - 1];
      const cands = nbrs(cx, cy);
      if (!cands.length) break;
      let best = cands[0];
      if (chain.length >= 2) {
        const [qx, qy] = chain[chain.length - 2];
        const dx = cx - qx;
        const dy = cy - qy;
        let bestDot = -Infinity;
        for (const c of cands) {
          const d = (c[0] - cx) * dx + (c[1] - cy) * dy;
          if (d > bestDot) {
            bestDot = d;
            best = c;
          }
        }
      }
      chain.push(best);
      visited[best[1] * cells + best[0]] = 1;
      if (chain.length > 20000) break;
    }
    return chain;
  };
  const chains: [number, number][][] = [];
  for (let y = 0; y < cells; y++) {
    for (let x = 0; x < cells; x++) {
      if (isRoad(x, y) && !visited[y * cells + x] && nbrs(x, y).length === 1) {
        chains.push(walkChain(x, y));
      }
    }
  }
  for (let y = 0; y < cells; y++) {
    for (let x = 0; x < cells; x++) {
      if (isRoad(x, y) && !visited[y * cells + x]) chains.push(walkChain(x, y));
    }
  }
  // 2) Chaikin 平滑 ×2（16m→4m 点距）→ 沿曲线铺路带
  const roadY = (wx: number, wy: number): number => {
    const ter = heightWorld(wx, wy);
    return Math.max(ter + 2.5, data.waterZ + 9);
  };
  const HW = 14; // 半宽（路宽 28m）
  let base = 0;
  for (const chain of chains) {
    if (chain.length < 2) continue;
    let pts: [number, number][] = chain.map(([x, y]) => [
      ORG[0] + (x + 0.5) * cellM,
      ORG[1] + (y + 0.5) * cellM,
    ]);
    for (let it = 0; it < 2; it++) {
      const out: [number, number][] = [pts[0]];
      for (let i = 0; i < pts.length - 1; i++) {
        const [ax, ay] = pts[i];
        const [bx, by] = pts[i + 1];
        out.push([ax * 0.75 + bx * 0.25, ay * 0.75 + by * 0.25]);
        out.push([ax * 0.25 + bx * 0.75, ay * 0.25 + by * 0.75]);
      }
      out.push(pts[pts.length - 1]);
      pts = out;
    }
    let distance = 0;
    let previousDistance = 0;
    let prevL: number[] | null = null;
    let prevR: number[] | null = null;
    for (let i = 0; i < pts.length; i++) {
      const [wx, wy] = pts[i];
      if (i > 0) distance += Math.hypot(wx - pts[i-1][0], wy - pts[i-1][1]);
      const z = roadY(wx, wy);
      const [fx, fy] = pts[Math.min(i + 1, pts.length - 1)];
      const [bx2, by2] = pts[Math.max(i - 1, 0)];
      let dx = fx - bx2;
      let dy = fy - by2;
      const len = Math.hypot(dx, dy) || 1;
      dx /= len;
      dy /= len;
      const L = [wx - dy * HW, z, wy + dx * HW];
      const R = [wx + dy * HW, z, wy - dx * HW];
      if (prevL && prevR) {
        rp.push(
          prevL[0], prevL[1], prevL[2],
          prevR[0], prevR[1], prevR[2],
          L[0], L[1], L[2],
          R[0], R[1], R[2],
        );
        for (let k = 0; k < 4; k++) rc.push(1, 1, 1);
        uv.push(0, previousDistance / 28, 1, previousDistance / 28, 0, distance / 28, 1, distance / 28);
        ri.push(base, base + 1, base + 2, base + 2, base + 1, base + 3);
        base += 4;
      }
      previousDistance = distance;
      prevL = L;
      prevR = R;
    }
  }
  if (base > 0) {
    const rgeo = new THREE.BufferGeometry();
    rgeo.setAttribute("position", new THREE.Float32BufferAttribute(rp, 3));
    rgeo.setAttribute("color", new THREE.Float32BufferAttribute(rc, 3));
    rgeo.setIndex(ri);
    rgeo.setAttribute("uv", new THREE.Float32BufferAttribute(uv, 2));
    // 平面路带：法线恒朝上，避免逐段法线抖动
    const rnorm = new Float32Array(rp.length);
    for (let i = 0; i < rnorm.length; i += 3) rnorm[i + 1] = 1;
    rgeo.setAttribute("normal", new THREE.BufferAttribute(rnorm, 3));
    group.add(
      new THREE.Mesh(
        rgeo,
        roadMaterial(),
      ),
    );
  }
  return group;
}

function roadMaterial(): THREE.MeshLambertMaterial {
  const canvas = document.createElement("canvas");
  canvas.width = 128; canvas.height = 128;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#535657"; ctx.fillRect(0, 0, 128, 128);
  ctx.fillStyle = "#97988f"; ctx.fillRect(0, 0, 10, 128); ctx.fillRect(118, 0, 10, 128);
  ctx.fillStyle = "#eee7cd";
  ctx.fillRect(14, 0, 2, 128); ctx.fillRect(112, 0, 2, 128);
  ctx.fillRect(40, 0, 2, 48); ctx.fillRect(86, 0, 2, 48);
  ctx.fillStyle = "#d1b966"; ctx.fillRect(61, 0, 2, 128); ctx.fillRect(65, 0, 2, 128);
  const map = new THREE.CanvasTexture(canvas);
  map.colorSpace = THREE.SRGBColorSpace;
  map.wrapS = map.wrapT = THREE.RepeatWrapping;
  map.anisotropy = 4;
  const material = new THREE.MeshLambertMaterial({ map, side: THREE.DoubleSide });
  material.addEventListener("dispose", () => map.dispose());
  return material;
}
