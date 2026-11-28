import { command } from "@/api/tauri";
export interface DomNode {
  i: number;
  parent: number;
  tag: string;
  id?: string;
  classes?: string;
  text?: string;
  rect?: number[];
}
export interface DomFrame {
  pid: number;
  time: number;
  sequence: number;
  viewport: number[];
  nodes?: DomNode[];
  boxes: DomNode[];
  truncated?: boolean;
}
export const readElements = (pid: number) =>
  command<DomFrame | null>("devtool_elements", { pid });
export function elementModel(frame: DomFrame) {
  const bounds = new Map(frame.boxes.map((n) => [n.i, n]));
  const nodes = (frame.nodes ?? frame.boxes).map((n) => ({
    ...n,
    ...bounds.get(n.i),
  }));
  const byId = new Map(nodes.map((n) => [n.i, n]));
  const children = new Map<number, DomNode[]>();
  for (const n of nodes) {
    const parent = byId.has(n.parent) && n.parent < n.i ? n.parent : -1;
    const group = children.get(parent) ?? [];
    group.push(n);
    children.set(parent, group);
  }
  return { nodes, byId, children };
}
