import type { LotUnitDto } from "@/api/tauri";
import type { AuthoredLot } from "@/lib/lot-ground/authored";

export interface BlendSceneDocument {
  format: "openscp.blend-scene/1";
  units: LotUnitDto[];
  lot: (AuthoredLot & { offset: [number, number] }) | null;
  errors: { object: string; detail: string }[];
}
export interface NativeBlendImport {
  asset: string;
  source?: "blend-native";
  cache?: "hit";
  scene?: { asset: string; document: BlendSceneDocument };
  sceneError?: string;
}
