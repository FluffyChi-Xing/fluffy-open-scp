export type TerrainBrushMode = "raise" | "level" | "lower";

export interface TerrainStroke {
  mode: TerrainBrushMode;
  x: number;
  y: number;
  radius: number;
  strength: number;
}
