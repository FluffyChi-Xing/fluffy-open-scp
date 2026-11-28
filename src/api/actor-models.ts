import type { Tgi } from "./tauri";
export interface ActorModelResolution {
  models: { packageId: number; tgi: Tgi }[];
  steps: {
    tgi: Tgi;
    via: string;
    status: string;
    parts: {
      bodies: number[];
      bodiesMax: number[];
      heads: number[];
      headsMax: number[];
      outfits: number[];
      outfitsMax: number[];
    } | null;
  }[];
}
