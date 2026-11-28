import type { SemanticKeyRef } from "./tauri";
export interface TransportDetails {
  effects: SemanticKeyRef[];
  sinks: SemanticKeyRef[];
  graphics: SemanticKeyRef[];
}
export interface ActorInfo {
  id: number;
  models: SemanticKeyRef[];
  templates: SemanticKeyRef[];
  appearances: {
    body: number | null;
    skin: number | null;
    skinMax: number | null;
    outfit: number | null;
    outfitMax: number | null;
  }[];
  speed: number | null;
  scale: number | null;
  animation: SemanticKeyRef | null;
  spawn: SemanticKeyRef | null;
  destroy: SemanticKeyRef | null;
  resourceBind: SemanticKeyRef | null;
}
export interface SceneInfo {
  actors: ActorInfo[];
  selfDestruct: boolean | null;
  recognized: boolean;
  issues: { property: number; expected: number; actual: number }[];
}
