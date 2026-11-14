/**
 * 逻辑编排图（2026-11-07）：建筑信息 / 行为聚合 / 行为叶子。
 *
 * 位置：artwork 工作台文档 `editor.workbench.logic`（与 menu/paths 同级）。
 * 构建器 `asset_build_m1/workbench_logic.rs` 是同一份契约的权威实现：
 * 这里只做作者侧的节点增删/连线/参数校验（纯函数），构建时由 Rust 编译成
 * `semantics.labor` + `semantics.behaviors` 并交给行为处理器。
 *
 * 语义边界（docs/re/asset-behavior-census-and-gating-20261106.md §4.2）：
 * 具名属性 hash 只是 UI/规则参数，真正的行为开关是 bundle/bin/sink/规则链。
 * 因此 `behavior-core` 的 inherit 档表示"沿用 donor 的闭环"，explicit 档才写
 * 数值并确保对应 utility bundle——不能只写数值。
 */

import { validCommercial, type CommercialParams } from "./commercialBehavior";
import { validOffice, type OfficeParams } from "./officeBehavior";
import { residenceCapacityError, type ResidenceParams } from "./residenceBehavior";
import behaviorPolicy from "@/lib/asset-behavior-policy.json";

export type LogicNodeKind = "building-info" | "behavior-core" | "behavior-leaf";
export type BehaviorLeafType =
  | "education"
  | "tourism"
  | "residence"
  | "office"
  | "commercial"
  | "hotel"
  | "park"
  | "tree"
  | "landvalue";
/** 逻辑节点挂到资产锚点/聚合节点上的端口。 */
export type LogicTargetHandle = "info" | "behavior" | "leaf";

export interface LogicNode {
  id: string;
  kind: LogicNodeKind;
  position: [number, number];
}

export interface LogicEdge {
  id: string;
  source: string;
  target: string;
  targetHandle: LogicTargetHandle;
}

export interface BuildingInfoParams {
  /** 这栋楼需要的劳工总数——作者唯一必须理解的概念。 */
  workers: number | null;
  /** 高级：按收入档拆分（三项齐全时覆盖 workers 的总数分配）。 */
  w1: number | null;
  w2: number | null;
  w3: number | null;
  /** 开工所需最低人数；null = 后端按原版比例（约 1/6）自动。 */
  minWorkers: number | null;
  /** 八班排班表（每班规模，0=关班）；班位 i 对应排班列 0x0D00853A + i。 */
  schedule: number[];
}

export type UtilityMode = "inherit" | "explicit";

export interface UtilityParams {
  mode: UtilityMode;
  /** 周期（仅电力有实证 hash 0x0AFB9882）。 */
  rate: number | null;
  /** 单次消耗量；Capacity 由构建器按 3×amount 派生。 */
  amount: number | null;
}

export interface BehaviorCoreParams {
  power: UtilityParams;
  water: UtilityParams;
  garbage: UtilityParams;
}

export interface EducationParams {
  type: "education";
  capacity: number;
  walkPath: boolean;
  servicePath: boolean;
  pedestrianPath: boolean;
  presentationPackage: string;
  presentationUnit: string;
  boardInstance: string;
  signatureBundles: string;
  removeBundles: string;
  titleOverride: string;
}

export interface TourismParams {
  type: "tourism";
  tourismProfile?: "native-tourism-v1";
  lodging?: boolean;
  attraction?: number | null;
  shoppers?: number | null;
  quotaW1?: number | null;
  quotaW2?: number | null;
  quotaW3?: number | null;
  incomeW1?: number | null;
  incomeW2?: number | null;
  incomeW3?: number | null;
  templateInstance?: string;
}

export const TOURISM_TEMPLATE_DEFAULTS = {
  attraction: 400,
  shoppers: 175,
  quotaW1: 100,
  quotaW2: 50,
  quotaW3: 25,
  incomeW1: 10,
  incomeW2: 10,
  incomeW3: 10,
} as const;

export interface ParkParams {
  type: "park";
  landValue: number;
  landWealth: number;
  landValue2: number;
  landWealth2: number;
  attraction: number;
}

export interface TreeParams {
  type: "tree";
  enabled: boolean;
}

export interface LandvalueParams {
  type: "landvalue";
  landValueProfile?: "copan-land-value-v1";
  value?: number | null;
  wealth?: number;
  value2?: number | null;
  wealth2?: number;
}

export const LAND_TEMPLATE_VALUE = 500;

export type LeafParams =
  | { type: "hotel"; rooms?: number | null }
  | EducationParams
  | TourismParams
  | ResidenceParams
  | OfficeParams
  | CommercialParams
  | ParkParams
  | TreeParams
  | LandvalueParams;
export type LogicParams =
  | BuildingInfoParams
  | BehaviorCoreParams
  | LeafParams;

export interface LogicGraph {
  nodes: LogicNode[];
  edges: LogicEdge[];
  params: Record<string, LogicParams>;
}

/** 节点目录（右键菜单/节点渲染共用；顺序 = 菜单顺序）。 */
export interface LogicNodeSpec {
  kind: LogicNodeKind;
  leafType?: BehaviorLeafType;
  icon: string;
  labelKey: string;
  hintKey: string;
  defaultParams: () => LogicParams;
}

const inheritUtility = (rate: number | null): UtilityParams => ({
  mode: "inherit",
  rate,
  amount: null,
});

export function defaultEducationParams(): EducationParams {
  return {
    type: "education",
    capacity: 250,
    walkPath: true,
    servicePath: true,
    pedestrianPath: false,
    presentationPackage: "",
    presentationUnit: "",
    boardInstance: "",
    signatureBundles: "",
    removeBundles: "",
    titleOverride: "",
  };
}

export const LOGIC_NODE_SPECS: readonly LogicNodeSpec[] = [
  {
    kind: "building-info",
    icon: "Users",
    labelKey: "studio.assetWorkbench.logicNode.buildingInfo",
    hintKey: "studio.assetWorkbench.logicNode.buildingInfoHint",
    defaultParams: () => ({
      workers: 25,
      w1: null,
      w2: null,
      w3: null,
      minWorkers: null,
      schedule: [6, 6, 7, 7, 6, 6, 7, 6],
    }),
  },
  {
    kind: "behavior-core",
    icon: "Gauge",
    labelKey: "studio.assetWorkbench.logicNode.behaviorCore",
    hintKey: "studio.assetWorkbench.logicNode.behaviorCoreHint",
    defaultParams: () => ({
      power: inheritUtility(72),
      water: inheritUtility(null),
      garbage: inheritUtility(null),
    }),
  },
  {
    kind: "behavior-leaf",
    leafType: "education",
    icon: "GraduationCap",
    labelKey: "studio.assetWorkbench.logicNode.leafEducation",
    hintKey: "studio.assetWorkbench.logicNode.leafEducationHint",
    defaultParams: defaultEducationParams,
  },
  {
    kind: "behavior-leaf",
    leafType: "tourism",
    icon: "Camera",
    labelKey: "studio.assetWorkbench.logicNode.leafTourism",
    hintKey: "studio.assetWorkbench.logicNode.leafTourismHint",
    defaultParams: () => ({
      type: "tourism",
      tourismProfile: "native-tourism-v1",
    }),
  },
  {
    kind: "behavior-leaf",
    leafType: "office",
    icon: "Building2",
    labelKey: "studio.assetWorkbench.logicNode.leafOffice",
    hintKey: "studio.assetWorkbench.logicNode.leafOfficeHint",
    defaultParams: () => ({ type: "office" }),
  },
  {
    kind: "behavior-leaf",
    leafType: "commercial",
    icon: "Store",
    labelKey: "studio.assetWorkbench.logicNode.leafCommercial",
    hintKey: "studio.assetWorkbench.logicNode.leafCommercialHint",
    defaultParams: () => ({ type: "commercial", commercialProfile: "ground-commercial-w2-v1" }),
  },
  {
    kind: "behavior-leaf",
    leafType: "hotel",
    icon: "BedDouble",
    labelKey: "studio.assetWorkbench.logicNode.leafHotel",
    hintKey: "studio.assetWorkbench.logicNode.leafHotelHint",
    defaultParams: () => ({ type: "hotel" }),
  },
  {
    kind: "behavior-leaf",
    leafType: "residence",
    icon: "House",
    labelKey: "studio.assetWorkbench.logicNode.leafResidence",
    hintKey: "studio.assetWorkbench.logicNode.leafResidenceHint",
    defaultParams: () => ({
      type: "residence",
      residenceProfile: "ground-residence-w1-v1",
    }),
  },
  {
    kind: "behavior-leaf",
    leafType: "park",
    icon: "Trees",
    labelKey: "studio.assetWorkbench.logicNode.leafPark",
    hintKey: "studio.assetWorkbench.logicNode.leafParkHint",
    defaultParams: () => ({
      type: "park",
      landValue: 500,
      landWealth: 3,
      landValue2: 500,
      landWealth2: 2,
      attraction: 500,
    }),
  },
  {
    kind: "behavior-leaf",
    leafType: "tree",
    icon: "TreePine",
    labelKey: "studio.assetWorkbench.logicNode.leafTree",
    hintKey: "studio.assetWorkbench.logicNode.leafTreeHint",
    defaultParams: () => ({ type: "tree", enabled: true }),
  },
  {
    kind: "behavior-leaf",
    leafType: "landvalue",
    icon: "TrendingUp",
    labelKey: "studio.assetWorkbench.logicNode.leafLandvalue",
    hintKey: "studio.assetWorkbench.logicNode.leafLandvalueHint",
    defaultParams: () => ({
      type: "landvalue",
      landValueProfile: "copan-land-value-v1",
    }),
  },
];

export function leafSpec(type: BehaviorLeafType): LogicNodeSpec {
  return (
    LOGIC_NODE_SPECS.find(
      (spec) => spec.kind === "behavior-leaf" && spec.leafType === type,
    ) ?? LOGIC_NODE_SPECS[0]
  );
}

export function emptyLogicGraph(): LogicGraph {
  return { nodes: [], edges: [], params: {} };
}

/**
 * 深拷贝纯 JSON 参数。不能用 structuredClone：Vue 的深响应式会把嵌套对象
 * 包成 Proxy，structuredClone(proxy) 会抛 DataCloneError（工作台白屏）。
 */
export function cloneLogicParams<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

export function isLeaf(node: LogicNode): boolean {
  return node.kind === "behavior-leaf";
}

export function leafTypeOf(
  graph: LogicGraph,
  node: LogicNode,
): BehaviorLeafType | null {
  if (!isLeaf(node)) return null;
  const params = graph.params[node.id] as LeafParams | undefined;
  return params?.type ?? null;
}

function nextId(prefix: string, existing: string[]): string {
  let max = 0;
  const pattern = new RegExp(`^${prefix}-(\\d+)$`);
  for (const id of existing) {
    const match = pattern.exec(id);
    if (match) max = Math.max(max, Number(match[1]));
  }
  return `${prefix}-${max + 1}`;
}

/** 节点前缀（id 可读且稳定；与 Rust 编译器的 id 无关，只做唯一键）。 */
function prefixFor(spec: LogicNodeSpec): string {
  if (spec.kind === "building-info") return "info";
  if (spec.kind === "behavior-core") return "core";
  return `leaf-${spec.leafType ?? "unknown"}`;
}

export function addLogicNode(
  graph: LogicGraph,
  spec: LogicNodeSpec,
  position: [number, number],
): { graph: LogicGraph; node: LogicNode } {
  const node: LogicNode = {
    id: nextId(prefixFor(spec), graph.nodes.map((item) => item.id)),
    kind: spec.kind,
    position,
  };
  return {
    graph: {
      nodes: [...graph.nodes, node],
      edges: graph.edges,
      params: { ...graph.params, [node.id]: spec.defaultParams() },
    },
    node,
  };
}

/** 确保恰好一个聚合节点（叶子必须有挂载点）。返回既有 core 或新建的 core。 */
export function ensureCoreNode(
  graph: LogicGraph,
  position: [number, number],
): { graph: LogicGraph; node: LogicNode } {
  const existing = graph.nodes.find((node) => node.kind === "behavior-core");
  if (existing) return { graph, node: existing };
  const spec = LOGIC_NODE_SPECS.find(
    (item) => item.kind === "behavior-core",
  )!;
  return addLogicNode(graph, spec, position);
}

export interface RemoveResult {
  graph: LogicGraph;
  /** 非空 = 拒绝删除（原因码由 UI 翻译）。 */
  blocked: string | null;
}

export function removeLogicNode(graph: LogicGraph, id: string): RemoveResult {
  const node = graph.nodes.find((item) => item.id === id);
  if (!node) return { graph, blocked: null };
  if (node.kind === "behavior-core") {
    const attached = graph.edges.some(
      (edge) => edge.target === id && edge.targetHandle === "leaf",
    );
    if (attached) return { graph, blocked: "coreHasLeaves" };
  }
  const params = { ...graph.params };
  delete params[id];
  return {
    graph: {
      nodes: graph.nodes.filter((item) => item.id !== id),
      edges: graph.edges.filter(
        (edge) => edge.source !== id && edge.target !== id,
      ),
      params,
    },
    blocked: null,
  };
}

export function duplicateLogicNode(
  graph: LogicGraph,
  id: string,
  offset: [number, number] = [40, 40],
): { graph: LogicGraph; node: LogicNode | null } {
  const node = graph.nodes.find((item) => item.id === id);
  const params = graph.params[id];
  if (!node || !params) return { graph, node: null };
  const copy: LogicNode = {
    id: nextId(`${id}-copy`, graph.nodes.map((item) => item.id)),
    kind: node.kind,
    position: [node.position[0] + offset[0], node.position[1] + offset[1]],
  };
  return {
    graph: {
      nodes: [...graph.nodes, copy],
      edges: graph.edges,
      params: { ...graph.params, [copy.id]: cloneLogicParams(params) },
    },
    node: copy,
  };
}

const HANDLE_BY_KIND: Record<Exclude<LogicNodeKind, "behavior-leaf">, LogicTargetHandle> = {
  "building-info": "info",
  "behavior-core": "behavior",
};

export interface ConnectResult {
  graph: LogicGraph;
  error: string | null;
}

/** 连线规则：info/core → asset 对应端口；leaf → core 的 leaf 端口。 */
export function connectLogic(
  graph: LogicGraph,
  source: string,
  target: string,
  targetHandle: LogicTargetHandle | null,
): ConnectResult {
  const node = graph.nodes.find((item) => item.id === source);
  if (!node) return { graph, error: "edgeDangling" };
  if (node.kind === "behavior-leaf") {
    const core = graph.nodes.find((item) => item.id === target);
    if (!core || core.kind !== "behavior-core" || targetHandle !== "leaf")
      return { graph, error: "leafRequiresCore" };
  } else {
    const expected = HANDLE_BY_KIND[node.kind];
    if (target !== "asset" || targetHandle !== expected)
      return { graph, error: "nodeRequiresAsset" };
    // 单例：info/core 各只允许一个，重复连线视为替换。
  }
  const others = graph.edges.filter((edge) => {
    if (edge.source === source) {
      // 每个 info/core 只保留一条锚点边；leaf 允许一条 leaf 边。
      return false;
    }
    return true;
  });
  const edge: LogicEdge = {
    id: `e-${source}-${target}-${targetHandle ?? ""}`,
    source,
    target,
    targetHandle: targetHandle ?? "leaf",
  };
  const exists = others.some(
    (item) =>
      item.source === edge.source &&
      item.target === edge.target &&
      item.targetHandle === edge.targetHandle,
  );
  return {
    graph: {
      ...graph,
      edges: exists ? others : [...others, edge],
    },
    error: null,
  };
}

export function disconnectLogic(graph: LogicGraph, edgeId: string): LogicGraph {
  return {
    ...graph,
    edges: graph.edges.filter((edge) => edge.id !== edgeId),
  };
}

export function updateLogicParams(
  graph: LogicGraph,
  id: string,
  patch: Record<string, unknown>,
): LogicGraph {
  const current = graph.params[id];
  if (!current) return graph;
  return {
    ...graph,
    params: { ...graph.params, [id]: { ...current, ...patch } as LogicParams },
  };
}

/** 校验问题（code + 责任节点；UI 按 code 出文案，不出现裸 hash）。 */
export interface LogicIssue {
  code: string;
  nodeId?: string;
}

const hexOk = (value: string): boolean =>
  typeof value === "string" &&
  (value.trim() === "" || /^0x[0-9a-f]{1,8}$/i.test(value.trim()));

function numberOk(value: unknown, min = 0): boolean {
  return typeof value === "number" && Number.isFinite(value) && value >= min;
}

/** 图级校验（fail-closed；构建器还会做一次同构校验）。 */
export function logicIssues(graph: LogicGraph): LogicIssue[] {
  const issues: LogicIssue[] = [];
  const leaves = graph.nodes.filter(isLeaf);
  for (const node of leaves.slice(behaviorPolicy.maxLeaves)) {
    issues.push({ code: "behaviorLeafLimit", nodeId: node.id });
  }
  for (const [index, node] of leaves.entries()) {
    const type = leafTypeOf(graph, node);
    if (type && leaves.slice(0, index).some((other) => {
      const previous = leafTypeOf(graph, other);
      return previous != null && behaviorTypesConflict(type, previous);
    })) {
      issues.push({ code: "behaviorLeafConflict", nodeId: node.id });
    }
  }
  const ids = new Set<string>();
  for (const node of graph.nodes) {
    if (ids.has(node.id)) issues.push({ code: "duplicateNode", nodeId: node.id });
    ids.add(node.id);
  }
  const infos = graph.nodes.filter((node) => node.kind === "building-info");
  const cores = graph.nodes.filter((node) => node.kind === "behavior-core");
  if (infos.length > 1)
    for (const node of infos.slice(1))
      issues.push({ code: "duplicateInfo", nodeId: node.id });
  if (cores.length > 1)
    for (const node of cores.slice(1))
      issues.push({ code: "duplicateCore", nodeId: node.id });

  for (const edge of graph.edges) {
    if (!ids.has(edge.source) || (edge.target !== "asset" && !ids.has(edge.target))) {
      issues.push({ code: "edgeDangling", nodeId: edge.source });
      continue;
    }
    const source = graph.nodes.find((node) => node.id === edge.source);
    if (!source) continue;
    if (source.kind === "behavior-leaf") {
      const target = graph.nodes.find((node) => node.id === edge.target);
      if (!target || target.kind !== "behavior-core" || edge.targetHandle !== "leaf")
        issues.push({ code: "leafRequiresCore", nodeId: source.id });
    } else if (
      edge.target !== "asset" ||
      edge.targetHandle !== HANDLE_BY_KIND[source.kind]
    ) {
      issues.push({ code: "nodeRequiresAsset", nodeId: source.id });
    }
  }

  for (const node of graph.nodes) {
    if (node.kind === "behavior-leaf") {
      const attached = graph.edges.some(
        (edge) => edge.source === node.id && edge.targetHandle === "leaf",
      );
      if (!attached) issues.push({ code: "leafUnattached", nodeId: node.id });
    } else {
      const handle = HANDLE_BY_KIND[node.kind];
      const attached = graph.edges.some(
        (edge) =>
          edge.source === node.id &&
          edge.target === "asset" &&
          edge.targetHandle === handle,
      );
      if (!attached) issues.push({ code: "nodeUnattached", nodeId: node.id });
    }
    const params = graph.params[node.id];
    if (!params) {
      issues.push({ code: "paramsMissing", nodeId: node.id });
      continue;
    }
    if (node.kind === "building-info") {
      const labor = params as BuildingInfoParams;
      const validCount = (v: unknown) => v === null || v === undefined || (Number.isInteger(v) && (v as number) >= 0);
      const tiersComplete = [labor.w1, labor.w2, labor.w3].every(
        (v) => Number.isInteger(v) && (v as number) >= 0,
      );
      const hasWorkers =
        Number.isInteger(labor.workers) && (labor.workers as number) >= 0;
      if (
        !validCount(labor.workers) ||
        !validCount(labor.w1) ||
        !validCount(labor.w2) ||
        !validCount(labor.w3) ||
        !validCount(labor.minWorkers) ||
        (!hasWorkers && !tiersComplete)
      )
        issues.push({ code: "laborInvalid", nodeId: node.id });
    } else if (node.kind === "behavior-core") {
      const core = params as BehaviorCoreParams;
      for (const [name, utility] of Object.entries(core)) {
        if (utility.mode !== "explicit") continue;
        if (!numberOk(utility.amount, 1))
          issues.push({ code: `coreAmount:${name}`, nodeId: node.id });
        if (utility.rate !== null && !numberOk(utility.rate))
          issues.push({ code: `coreRate:${name}`, nodeId: node.id });
      }
    } else {
      const leaf = params as LeafParams;
      switch (leaf.type) {
        case "education":
          if (!numberOk(leaf.capacity, 1))
            issues.push({ code: "educationCapacity", nodeId: node.id });
          if (!hexOk(leaf.boardInstance))
            issues.push({ code: "leafHashInvalid", nodeId: node.id });
          if (!hexOk(leaf.presentationUnit))
            issues.push({ code: "leafHashInvalid", nodeId: node.id });
          break;
        case "tourism":
          if (leaf.attraction != null && !numberOk(leaf.attraction))
            issues.push({ code: "tourismAttraction", nodeId: node.id });
          if (!leaf.tourismProfile && leaf.templateInstance != null && !hexOk(leaf.templateInstance))
            issues.push({ code: "leafHashInvalid", nodeId: node.id });
          break;
        case "park":
          if (!numberOk(leaf.landValue) || !numberOk(leaf.landValue2))
            issues.push({ code: "landValueInvalid", nodeId: node.id });
          break;
        case "landvalue":
          if (leaf.landValueProfile
            ? [leaf.value, leaf.value2].some((value) => value != null && (!Number.isFinite(value) || Math.abs(value) > 100_000))
            : !numberOk(leaf.value) || !numberOk(leaf.value2))
            issues.push({ code: "landValueInvalid", nodeId: node.id });
          break;
        case "residence": {
          const error = residenceCapacityError(leaf);
          if (error)
            issues.push({ code: error === "populationInvalid" ? "residenceCapacity" : "residenceSlots", nodeId: node.id });
          break;
        }
        case "commercial":
          if (!validCommercial(leaf)) issues.push({ code: "commercialInvalid", nodeId: node.id });
          break;
        case "hotel":
          if (!validHotelRooms(leaf.rooms)) issues.push({ code: "hotelRooms", nodeId: node.id });
          break;
        case "office":
          if (!validOffice(leaf)) issues.push({ code: "officeInvalid", nodeId: node.id });
          break;
        case "tree":
          break;
        default:
          issues.push({ code: "leafTypeUnknown", nodeId: node.id });
      }
    }
  }
  return issues;
}

export const MAX_BEHAVIOR_LEAVES = behaviorPolicy.maxLeaves;

export function behaviorTypesConflict(first: BehaviorLeafType, second: BehaviorLeafType): boolean {
  return first === second || behaviorPolicy.incompatible.some(([head, ...others]) =>
    (head === first && others.includes(second)) || (head === second && others.includes(first)),
  );
}

export function validHotelRooms(rooms: unknown): boolean {
  return rooms == null || (typeof rooms === "number" && Number.isInteger(rooms) && rooms >= 1 && rooms <= 1000);
}

// ---- 文档迁移（旧文档没有 logic 块 → 空图） ----

function parsePosition(raw: unknown): [number, number] | null {
  if (!Array.isArray(raw) || raw.length !== 2) return null;
  const [x, y] = raw;
  if (typeof x !== "number" || typeof y !== "number") return null;
  if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
  return [x, y];
}

/** 从文档恢复（防御脏数据；边引用不存在节点时丢弃）。 */
export function normalizeLogicGraph(raw: unknown): LogicGraph {
  const graph = emptyLogicGraph();
  if (typeof raw !== "object" || raw === null) return graph;
  const record = raw as Record<string, unknown>;
  const seen = new Set<string>();
  if (Array.isArray(record.nodes)) {
    for (const value of record.nodes) {
      if (typeof value !== "object" || value === null) continue;
      const node = value as Record<string, unknown>;
      const id = typeof node.id === "string" ? node.id.trim() : "";
      const kind = node.kind;
      const position = parsePosition(node.position);
      if (!id || seen.has(id) || !position) continue;
      if (kind !== "building-info" && kind !== "behavior-core" && kind !== "behavior-leaf")
        continue;
      seen.add(id);
      graph.nodes.push({ id, kind, position });
    }
  }
  if (Array.isArray(record.edges)) {
    const edgeIds = new Set<string>();
    for (const value of record.edges) {
      if (typeof value !== "object" || value === null) continue;
      const edge = value as Record<string, unknown>;
      const id = typeof edge.id === "string" ? edge.id.trim() : "";
      const source = typeof edge.source === "string" ? edge.source : "";
      const target = typeof edge.target === "string" ? edge.target : "";
      const handle = edge.targetHandle;
      if (!id || edgeIds.has(id) || !seen.has(source)) continue;
      if (target !== "asset" && !seen.has(target)) continue;
      if (handle !== "info" && handle !== "behavior" && handle !== "leaf")
        continue;
      edgeIds.add(id);
      graph.edges.push({
        id,
        source,
        target,
        targetHandle: handle,
      });
    }
  }
  if (typeof record.params === "object" && record.params !== null) {
    for (const [id, params] of Object.entries(record.params)) {
      if (!seen.has(id) || typeof params !== "object" || params === null) continue;
      graph.params[id] = params as LogicParams;
    }
  }
  // 旧图迁移（2026-11-11）：老图只有 w1/w2/w3/minWorkers。补 workers 总额，
  // 让新的「需要的劳工数」主输入能显示出来；分档原样保留为高级覆盖。
  for (const node of graph.nodes) {
    if (node.kind !== "building-info") continue;
    const params = graph.params[node.id] as unknown as Record<string, unknown>;
    if (!params || params.workers !== undefined) continue;
    const tiers = ["w1", "w2", "w3"].map((key) => params[key]);
    if (tiers.every((value) => typeof value === "number")) {
      params.workers = (tiers as number[]).reduce((sum, value) => sum + value, 0);
    }
  }
  // 参数缺失的节点补默认值（按 kind / leaf type），避免图可画但构建必失败。
  for (const node of graph.nodes) {
    if (graph.params[node.id]) continue;
    if (node.kind === "building-info")
      graph.params[node.id] = {
        workers: 25,
        w1: null,
        w2: null,
        w3: null,
        minWorkers: null,
        schedule: [6, 6, 7, 7, 6, 6, 7, 6],
      };
    else if (node.kind === "behavior-core")
      graph.params[node.id] = {
        power: inheritUtility(72),
        water: inheritUtility(null),
        garbage: inheritUtility(null),
      };
    else graph.params[node.id] = defaultEducationParams();
  }
  return graph;
}
