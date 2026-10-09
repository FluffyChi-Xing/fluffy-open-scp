import type { LotMaterialTextures, LotModelPayload } from "@/api/tauri";

export const BUILDING_SLOTS = [
  {
    id: "slot0",
    label: "参数表",
    hint: "JSON：cols 与 values（4 × cols × float4）",
    accept: ["json"],
  },
  {
    id: "slot1",
    label: "颜色控制图",
    hint: "RGBA 控制图，保留 Alpha",
    accept: ["png"],
  },
  {
    id: "slot2",
    label: "法线 / AO",
    hint: "线性 RGB 法线与 Alpha 通道",
    accept: ["png"],
  },
  {
    id: "slot3",
    label: "Shader Map",
    hint: "高光、窗洞与内景遮罩",
    accept: ["png"],
  },
  { id: "slot4", label: "调色板", hint: "建筑配色变体查找表", accept: ["png"] },
  {
    id: "slot5",
    label: "室内图集",
    hint: "预渲染房间与窗灯 Alpha",
    accept: ["png"],
  },
] as const;
export interface ProjectAsset {
  asset: string;
  path: string;
  size: number;
  base64?: string;
  resources?: import("@/api/tauri").Tgi[];
}
export function assetBytes(asset: ProjectAsset): Uint8Array<ArrayBuffer> {
  return Uint8Array.from(atob(asset.base64 ?? ""), (c) => c.charCodeAt(0));
}
export function applyBuildingSlots(
  payload: LotModelPayload,
  inputs: (ProjectAsset | null)[],
  materialIndex = 0,
): LotModelPayload {
  const material = payload.materials[materialIndex];
  if (!material) throw new Error("该模型没有可绑定的材质");
  if (
    inputs.some(Boolean) &&
    !material.paramCols &&
    !payload.meshUvKinds.some(
      (kind, index) =>
        kind === 2 && payload.meshMaterialIndices[index] === materialIndex,
    )
  ) {
    throw new Error(
      "六槽预设适用于 building4 立面材质；当前材质需要对应 shader 的专用适配",
    );
  }
  const next: LotMaterialTextures = { ...material };
  for (let i = 0; i < inputs.length; i++) {
    const asset = inputs[i];
    if (!asset) continue;
    const bytes = assetBytes(asset);
    if (i === 0) {
      const params = JSON.parse(new TextDecoder().decode(bytes)) as {
        cols: number;
        values: number[];
      };
      if (
        !Number.isInteger(params.cols) ||
        params.cols < 1 ||
        params.cols > 4096 ||
        !Array.isArray(params.values) ||
        params.values.length !== params.cols * 16 ||
        !params.values.every(Number.isFinite)
      )
        throw new Error("参数表需要 cols 和长度为 cols × 16 的有限数值 values");
      // Geometry stores column indices. Silently narrowing the table corrupts its material mapping.
      if (material.paramCols && params.cols < material.paramCols)
        throw new Error("参数表列数不能少于模型引用的列数");
      next.paramsF32 = new Float32Array(params.values);
      next.paramCols = params.cols;
    } else {
      if (
        !asset.asset.toLowerCase().endsWith(".png") ||
        ![137, 80, 78, 71, 13, 10, 26, 10].every(
          (byte, index) => bytes[index] === byte,
        )
      )
        throw new Error("建筑数据贴图必须使用无损 PNG");
      const fields = [
        "",
        "tintPng",
        "normalPng",
        "shaderPng",
        "palettePng",
        "interiorPng",
      ] as const;
      const key = fields[i];
      if (key) next[key] = bytes;
      if (i === 1 && !next.paramCols) next.baseColorPng = bytes;
    }
  }
  return {
    ...payload,
    materials: payload.materials.map((m, i) =>
      i === materialIndex ? next : m,
    ),
  };
}
