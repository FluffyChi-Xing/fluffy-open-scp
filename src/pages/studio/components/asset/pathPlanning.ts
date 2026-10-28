import type { LotUnitDto } from "@/api/tauri";

/**
 * 工作台路径规划纯函数（2026-10-27 path 研究落地，见
 * docs/re/workbench-path-planning-design-20261027.md）。
 *
 * 引擎语义锚点（CreatePathsFromProps，dev 0x7E3A50）：
 * - 点/切线两列必须等长，否则整批拒绝；
 * - pairs (start,end) 含端点，对数 = 路径数，奇数即错；
 * - path entry 不可解析 → 该路径整条跳过（编辑器必须把"无 entry"标红）；
 * - 切线写进段两端，供曲线位置/速度计算——威利斯实证切线随段长走，
 *   自动重算采用 Catmull-Rom 中点式 t_i = (p_{i+1} − p_{i−1})/2，
 *   端点取单侧弦向。
 */

export type Vec3 = [number, number, number];

/** 编辑态路径实体：几何在 pathPoint 单元上，这里只持有序 unitId 与语义。 */
export interface WorkbenchPath {
  id: string;
  label: string;
  /** path entry instance 的 hex（无 0x 前缀）；null = 未绑定（引擎跳过整条）。 */
  entry: string | null;
  scope: string;
  roadConnect: boolean;
  /** 有序 pathPoint unitId（pathPoint:{index}）。 */
  pointIds: string[];
}

export function nextPathId(existing: string[]): string {
  let max = 0;
  for (const id of existing) {
    const match = /^path:(\d+)$/.exec(id);
    if (match) max = Math.max(max, Number(match[1]));
  }
  return `path:${max + 1}`;
}

const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const half = (a: Vec3): Vec3 => [a[0] / 2, a[1] / 2, a[2] / 2];

/** Catmull-Rom 中点切线：t_i = (p_{i+1} − p_{i−1})/2；端点取单侧弦向。 */
export function pathAutoTangent(
  prev: Vec3 | null,
  point: Vec3,
  next: Vec3 | null,
): Vec3 {
  if (prev && next) return half(sub(next, prev));
  if (next) return sub(next, point);
  if (prev) return sub(point, prev);
  return [0, 0, 0];
}

/** 全路径切线重算：输入有序点位置，输出与输入等长的切线数组（引擎要求等长）。 */
export function recomputePathTangents(points: (Vec3 | null)[]): (Vec3 | null)[] {
  return points.map((point, index) => {
    if (!point) return null;
    return pathAutoTangent(
      points[index - 1] ?? null,
      point,
      points[index + 1] ?? null,
    );
  });
}

/** 校验问题码（面板按码显示文案；entry-missing 是引擎级行为开关）。 */
export type PathIssue =
  | "entry-missing"
  | "points-too-few"
  | "point-missing"
  | "tangent-missing";

export function validateWorkbenchPath(
  path: WorkbenchPath,
  points: (LotUnitDto | null)[],
): PathIssue[] {
  const issues: PathIssue[] = [];
  if (!path.entry) issues.push("entry-missing");
  if (points.filter(Boolean).length < 2) issues.push("points-too-few");
  for (const point of points) {
    if (!point) issues.push("point-missing");
    else if (point.kind === "pathPoint" && !point.tangent)
      issues.push("tangent-missing");
  }
  return issues;
}

/** 点位序列换算：unitId → point 坐标（缺单元给 null，校验用）。 */
export function pathPointPositions(
  path: WorkbenchPath,
  byId: Map<string, LotUnitDto>,
): (Vec3 | null)[] {
  return path.pointIds.map((id) => {
    const unit = byId.get(id);
    if (unit && unit.kind === "pathPoint" && unit.point) return unit.point;
    return null;
  });
}

/**
 * 孤儿点收养：文档里存在 pathPoint 单元但没有任何路径实体引用它们时
 * （历史会话只落了点、实体通道丢失），按 pointIndex 排序收养进一条
 * 新路径。无孤儿返回 null。恢复链每次重建文档时调用，幂等。
 */
export function adoptOrphanPaths(
  paths: WorkbenchPath[],
  units: LotUnitDto[],
): WorkbenchPath[] | null {
  const referenced = new Set(paths.flatMap((path) => path.pointIds));
  const orphans = units
    .filter(
      (unit): unit is LotUnitDto & { pointIndex: number | null } =>
        unit.kind === "pathPoint" && !referenced.has(`pathPoint:${unit.index}`),
    )
    .sort((a, b) => (a.pointIndex ?? a.index) - (b.pointIndex ?? b.index));
  if (!orphans.length) return null;
  let max = 0;
  const count = paths.length;
  for (const path of paths) {
    const match = /^path:(\d+)$/.exec(path.id);
    if (match) max = Math.max(max, Number(match[1]));
  }
  return [
    ...paths,
    {
      id: `path:${max + 1}`,
      label: `#${count + 1}`,
      entry: null,
      scope: "local",
      roadConnect: false,
      pointIds: orphans.map((unit) => `pathPoint:${unit.index}`),
    },
  ];
}
