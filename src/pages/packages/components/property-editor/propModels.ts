import type * as ThreeNamespace from "three";
import {
  pngBlobUrl,
  parseLotModelObjects,
  markGeometryShared,
} from "@/lib/three-gltf";
import type { LotModelPayload } from "@/api/tauri";
import {
  loadTintTextures,
  makeTintMaterial,
  type SunEnvRefs,
  type TintTextureSet,
} from "./refinedRender";

/**
 * PE 精细替换（P2）：prop 组件的真实模型装配。
 *
 * 数据链：lot prop 列 resourceID → 后端 resolve_prop_models（脚本资源表两分支，
 * 见 src-tauri prop_models.rs）→ read_lot_model_meshes RW4→LOTM 载荷 →
 * 本模块按材质装配。
 *
 * 材质按 uvKind 分流（与建筑装配同判据）：prop/车辆网格常带 facade FLOAT4
 * 顶点流 → uvKind=2 → tint 调色链（slot0 参数表 + tint/palette 查表，车漆
 * 颜色的来源；baseColorPng 此时为 null）；0/1 → baseColor 直贴。
 *
 * 调色板变体行（buildingVariation 引擎口径）：颜色查表 V = 变体整数行 +
 * tint.g 行内小数。建筑恒行 0（已对拍口径）；prop 按 unit.index 确定性取
 * 非空行（调色板纹理逐行解码，跳过全黑行）——同资源多 prop 呈现不同车漆，
 * 复刻游戏停车场的多色观感。每 prop 实例新鲜建 tint 材质（three 的
 * Material.clone 不拷贝 onBeforeCompile，不能克隆注入材质）；编译按
 * customProgramCacheKey 复用，成本是材质对象而非着色器。
 */

/**
 * 临时车漆色带（引擎 paintPaletteSampler 的近似替代）：全局色带纹理尚未
 * 定位（材质 slot0 引用解析为 model 类型条目，真图待查），先以经典城市
 * 车色数组按 U 确定性取样。引擎公式（vehicleSetupSHParams 逐字）：
 *   paintColor = paintPalette(miscParams.x, 0)
 *   diffuse    = lerp(paintColor, diffuseTex.rgb, diffuseTex.a)
 * diffuse alpha 即车体/细节遮罩（车体≈0 纯车漆、细节≈1 保贴图）。
 */
const PAINT_STRIP: [number, number, number][] = [
  [0.92, 0.92, 0.9], // 白
  [0.75, 0.76, 0.78], // 银
  [0.16, 0.17, 0.19], // 黑
  [0.55, 0.57, 0.6], // 灰
  [0.72, 0.16, 0.14], // 红
  [0.38, 0.1, 0.1], // 深红
  [0.16, 0.28, 0.55], // 蓝
  [0.1, 0.14, 0.32], // 藏青
  [0.2, 0.45, 0.2], // 绿
  [0.14, 0.36, 0.34], // 青绿
  [0.78, 0.6, 0.14], // 黄
  [0.65, 0.34, 0.12], // 橙
];

interface PropTemplateEntry {
  roots: ThreeNamespace.Object3D[];
  tintSets: TintTextureSet[];
  /** 该材质下标是否被任一 mesh 以 uvKind=2 使用。 */
  tintUsed: boolean[];
  /** 非 tint 材质（baseColor 直贴），跨实例共享。 */
  baseMaterials: ThreeNamespace.MeshStandardMaterial[];
  /** 每材质的非空调色板行（由 paletteTex 解码）。 */
  paletteRows: number[][];
  meshMaterialIndices: number[];
}

/** 载荷级模板缓存：parse 一次，clone 共享几何入场。 */
const templateCache = new WeakMap<LotModelPayload, PropTemplateEntry>();

/** blob URL 登记（模板存活期与 payload 一致，WeakMap 跟随回收）。 */
const blobUrls = new WeakMap<LotModelPayload, string[]>();

async function loadTexture(
  THREE: typeof ThreeNamespace,
  bytes: Uint8Array<ArrayBuffer> | null,
  urls: string[],
  srgb: boolean,
): Promise<ThreeNamespace.Texture | null> {
  if (!bytes) return null;
  const url = pngBlobUrl(bytes);
  urls.push(url);
  const texture = await new THREE.TextureLoader().loadAsync(url);
  if (srgb) texture.colorSpace = THREE.SRGBColorSpace;
  texture.flipY = false;
  texture.anisotropy = 4;
  return texture;
}

/**
 * 调色板非空行解码：纹理 512×16 = 8 行 × 2px，逐行采样找非全黑行。
 * 空行（全黑/全透明）是未用的变体槽；采样自 paletteTex.image（已按
 * colorSpaceConversion:none 解码的原始值，黑判定不受 sRGB 影响）。
 */
function decodePaletteRows(texture: ThreeNamespace.Texture | null): number[] {
  const image = texture?.image as
    | (TexImageSource & { width: number; height: number })
    | undefined;
  if (!image?.width || !image?.height) return [0];
  const canvas = document.createElement("canvas");
  canvas.width = image.width;
  canvas.height = image.height;
  const context = canvas.getContext("2d");
  if (!context) return [0];
  context.drawImage(image as CanvasImageSource, 0, 0);
  const { data } = context.getImageData(0, 0, canvas.width, canvas.height);
  const rows = 8;
  const rowHeight = canvas.height / rows;
  const nonEmpty: number[] = [];
  for (let row = 0; row < rows; row += 1) {
    let lit = false;
    for (
      let y = Math.floor(row * rowHeight);
      y < Math.floor((row + 1) * rowHeight) && !lit;
      y += 1
    ) {
      for (let x = 0; x < canvas.width && !lit; x += 4) {
        const at = (y * canvas.width + x) * 4;
        if (data[at + 3] > 8 && data[at] + data[at + 1] + data[at + 2] > 24) {
          lit = true;
        }
      }
    }
    if (lit) nonEmpty.push(row);
  }
  return nonEmpty.length > 0 ? nonEmpty : [0];
}

/**
 * 取 prop 载荷的模型对象（模板缓存命中时按实例建材质）。
 * variantSeed = unit.index（同资源多 prop 确定性分散调色板行）。
 */
export async function getPropModelObject(
  THREE: typeof ThreeNamespace,
  payload: LotModelPayload,
  env?: SunEnvRefs,
  variantSeed = 0,
): Promise<ThreeNamespace.Object3D | null> {
  let entry = templateCache.get(payload);
  if (!entry) {
    let roots: ThreeNamespace.Object3D[];
    try {
      roots = await parseLotModelObjects(payload.glbs);
    } catch {
      return null;
    }
    // UV 修正：GLTF 导出器按建筑 tint 链约定写 V'=-V（该链靠着色器 frac()
    // 回绕负 V 存活）；prop 是传统 UV0 直采，负 V 被 ClampToEdge 钳成单排
    // 纹素（整车灰色的根因）。几何为本模块新鲜解析，就地翻回。
    for (const root of roots) {
      root.traverse((child) => {
        const mesh = child as ThreeNamespace.Mesh;
        if (!mesh.isMesh) return;
        const uv = mesh.geometry.attributes.uv as
          | ThreeNamespace.BufferAttribute
          | undefined;
        if (!uv) return;
        for (let i = 0; i < uv.count; i += 1) {
          uv.setY(i, -uv.getY(i));
        }
        uv.needsUpdate = true;
      });
    }
    const urls: string[] = [];
    const tintSets = await loadTintTextures(
      THREE,
      payload.materials ?? [],
      4,
      urls,
    );
    // 该材质是否有 mesh 以 uvKind=2 使用（与建筑装配同判据）
    const tintUsed = (payload.materials ?? []).map((_m, materialIndex) =>
      (payload.meshUvKinds ?? []).some(
        (kind, meshIdx) =>
          kind === 2 &&
          (payload.meshMaterialIndices[meshIdx] ?? 0) === materialIndex,
      ),
    );
    const baseMaterials = (payload.materials ?? []).map(() => {
      return new THREE.MeshStandardMaterial({
        roughness: 0.8,
        metalness: 0,
        side: THREE.DoubleSide,
      });
    });
    // 非 tint 材质补 baseColor/normal 贴图（tint 材质自带贴图链）
    await Promise.all(
      (payload.materials ?? []).map(async (material, index) => {
        if (tintUsed[index]) return;
        const standard = baseMaterials[index];
        if (!standard) return;
        // slot0 = 车辆/prop 槽位语义的彩色漫反射（v10 下发）；缺失时
        // 退 baseColor（simple diffuse 建筑语义）。灰车根因即建筑槽位
        // 语义误用 slot1（车辆的灰度副本）。
        const diffuse = material.slot0Png ?? material.baseColorPng;
        standard.map = await loadTexture(THREE, diffuse, urls, true);
        standard.normalMap = await loadTexture(
          THREE,
          material.normalPng,
          urls,
          false,
        );
        if (standard.map) standard.color.set(0xffffff);
        standard.needsUpdate = true;
      }),
    );
    entry = {
      roots,
      tintSets,
      tintUsed,
      baseMaterials,
      paletteRows: tintSets.map((set) => decodePaletteRows(set.paletteTex)),
      meshMaterialIndices: payload.meshMaterialIndices,
    };
    blobUrls.set(payload, urls);
    templateCache.set(payload, entry);
    // 模板几何打共享标记：clone 入场后场景清场（disposeObject）不销毁，
    // 生命周期跟随模板缓存（payload 更换由 GC + WeakMap 回收 JS 侧）。
    for (const root of roots) {
      root.traverse((child) => {
        const mesh = child as ThreeNamespace.Mesh;
        if (mesh.isMesh && mesh.geometry) markGeometryShared(mesh.geometry);
      });
    }
  }
  const clone = entry.roots.map((root) => root.clone());
  if (clone.length !== 1) return null;
  // 逐 mesh 配材质：tint 材质每实例新鲜建（行 uniform 独立），baseColor
  // 材质每实例新鲜建（paint lerp 的 uPaintColor 独立）；贴图/着色器编译
  // 均按载荷/缓存键共享。
  const paintColor =
    PAINT_STRIP[Math.floor(variantSeed * 0.618) % PAINT_STRIP.length];
  let meshIndex = 0;
  clone[0].traverse((child) => {
    const mesh = child as ThreeNamespace.Mesh;
    if (!mesh.isMesh) return;
    const materialIndex = entry.meshMaterialIndices[meshIndex] ?? 0;
    if (entry.tintUsed[materialIndex] && env) {
      const tint = entry.tintSets[materialIndex];
      if (tint.tintTex && tint.paletteTex) {
        const [tinted] = makeTintMaterial(THREE, tint, env);
        const rows = entry.paletteRows[materialIndex];
        const rowUniform = tinted.userData.uPaletteRow as { value: number };
        rowUniform.value = rows[variantSeed % rows.length];
        mesh.material = tinted;
      }
    } else {
      const template = entry.baseMaterials[materialIndex];
      if (template) {
        const material = template.clone();
        const paint = { value: new THREE.Color(...paintColor) };
        material.userData.uPaintColor = paint;
        material.onBeforeCompile = (shader) => {
          shader.uniforms.uPaintColor = paint;
          shader.fragmentShader = shader.fragmentShader.replace(
            "#include <map_fragment>",
            `#include <map_fragment>
  // 引擎 vehicleSetupSHParams：车体(diffuse a≈0)用 paintColor，细节(a≈1)保贴图
  #ifdef USE_MAP
  diffuseColor.rgb = mix(uPaintColor, diffuseColor.rgb, diffuseColor.a);
  #endif`,
          );
          shader.fragmentShader = shader.fragmentShader.replace(
            "#include <common>",
            `#include <common>
uniform vec3 uPaintColor;`,
          );
        };
        material.customProgramCacheKey = () => "prop-paint";
        mesh.material = material;
      }
    }
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    meshIndex += 1;
  });
  return clone[0];
}
