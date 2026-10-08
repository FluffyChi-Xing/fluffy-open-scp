export interface FlowNode {
  id: string;
  schema: string;
  kind: string;
  position: [number, number];
}
export interface FlowSchema {
  schema_version: number;
  id: string;
  kind: string;
  config: Record<string, string | number>;
}
export interface FlowDiagnostic {
  node: string | null;
  code: string;
  detail: string;
}
export interface FlowIM {
  version: number;
  manifest_hash: string;
  order: string[];
  schemas: FlowSchema[];
  inputs: Record<string, string>;
}
export interface FlowState {
  manifest: Record<string, unknown>;
  revision: string;
  nodes: FlowNode[];
  schemas: FlowSchema[];
  diagnostics: FlowDiagnostic[];
  im: FlowIM | null;
}
export function isEngineManifest(value: Record<string, unknown>): boolean {
  const engine = value.engine as
    { enabled?: boolean; version?: number } | undefined;
  return (
    value.format === "openscp.mod" &&
    value.manifest_version === 2 &&
    value.origin === "openscp" &&
    engine?.enabled === true &&
    engine.version === 1
  );
}
