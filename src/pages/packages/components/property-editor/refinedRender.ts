import { pngBlobUrl } from "@/lib/three-gltf";
import type * as ThreeNamespace from "three";
import type { LotModelPayload } from "@/api/tauri";

/**
 * SCP 精细渲染模块（scene contributor）：building4 tint 着色器注入、
 * 5d 日/夜环境 uniform、tint 贴图预载与材质装配、逐 mesh 材质贴图绑定。
 * 从 PropertyEditorViewport 原地抽出（PE-重构-1），逻辑未变。
 */

type LotMaterial = NonNullable<LotModelPayload["materials"]>[number];

/**
 * specularity 通道：经多 package 比对（用户结论 2026-09-10），强制 B 通道
 * （窗）综合效果最佳，精细渲染固定使用；通道实验 UI 已停用（见
 * PropertyEditor.vue 注释），specMode=2 保留为将来复验的常量。
 */
export function effectiveSpecMode(): number {
  return 2;
}

/**
 * 5d 日/夜环境 uniform 共享实例（一次 rebuild 一组，全部 tint 材质引用同一
 * 对象，时段/供电变化直接改写免重建）。着色器侧 uSunDir 为世界空间。
 */
export type SunEnvRefs = {
  sunDir: { value: ThreeNamespace.Vector3 };
  sunColor: { value: ThreeNamespace.Color };
  /** 天顶色（引擎 SkyColorConv 的近似三段之一）。 */
  skyColor: { value: ThreeNamespace.Color };
  /** 地平线色（低仰角偏亮偏暖）。 */
  skyHorizon: { value: ThreeNamespace.Color };
  /** 地面反照（朝下的法线采到的环境色）。 */
  skyGround: { value: ThreeNamespace.Color };
  /** 方向性环境因子（scSkyRadiance 亮度 / 球面均值）的归一化基准。 */
  skyLumRef: { value: number };
  dayLight: { value: number };
  powered: { value: number };
  glow: { value: number };
};

/** 太阳地平线高度 −1..1（t=6/18 日出日落、12 正午、0/24 子夜）。 */
export const sunAltitude = (t: number) => Math.sin(((t - 6) / 12) * Math.PI);

/** 白昼因子 0..1（含晨昏过渡带）。 */
export const dayFactor = (timeOfDay: number) => {
  const alt = sunAltitude(timeOfDay);
  return Math.min(1, Math.max(0, (alt + 0.08) / 0.5));
};

const SAT = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v);
const smoothstep = (e0: number, e1: number, x: number) => {
  const t = SAT((x - e0) / (e1 - e0));
  return t * t * (3 - 2 * t);
};
const LUM = (c: [number, number, number]) =>
  0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];

type SkyEnv = Pick<
  SunEnvRefs,
  "sunDir" | "sunColor" | "skyColor" | "skyHorizon" | "skyGround"
>;

/**
 * 解析天空辐照（引擎 `SkyColorConv` 的近似）：天顶→地平三段渐变 + 太阳邻近辉光
 * + 地平线以下的地面反照。**与着色器 `scSkyRadiance` 必须逐式一致**。
 */
export function skyRadiance(
  dir: [number, number, number],
  env: SkyEnv,
): [number, number, number] {
  const up = Math.max(-1, Math.min(1, dir[2]));
  const zenith = env.skyColor.value.toArray() as [number, number, number];
  const horizon = env.skyHorizon.value.toArray() as [number, number, number];
  const ground = env.skyGround.value.toArray() as [number, number, number];
  const sun = env.sunColor.value.toArray() as [number, number, number];
  const sunDot = Math.max(
    0,
    dir[0] * env.sunDir.value.x +
      dir[1] * env.sunDir.value.y +
      dir[2] * env.sunDir.value.z,
  );
  const glow = Math.pow(sunDot, 8) * 0.3;
  const t = Math.pow(SAT(up), 0.45);
  const blend = smoothstep(-0.15, 0.05, up);
  const out: [number, number, number] = [0, 0, 0];
  for (let i = 0; i < 3; i += 1) {
    const sky = horizon[i] + (zenith[i] - horizon[i]) * t + sun[i] * glow;
    out[i] = ground[i] + (sky - ground[i]) * blend;
  }
  return out;
}

/** 固定 Fibonacci 球面采样（确定性：同一 env 恒得同一基准亮度）。 */
const SKY_SAMPLES: [number, number, number][] = Array.from(
  { length: 96 },
  (_, i) => {
    const z = 1 - (2 * (i + 0.5)) / 96;
    const r = Math.sqrt(Math.max(0, 1 - z * z));
    const theta = Math.PI * (1 + Math.sqrt(5)) * i;
    return [r * Math.cos(theta), r * Math.sin(theta), z];
  },
);

/**
 * 球面平均天空亮度。着色器用 `scSkyRadiance(法线) / uSkyLumRef` 调制间接漫反射，
 * 归一化到**均值 1** —— 只重新分配各朝向的环境光，不改变整体曝光。
 */
export function averageSkyLuminance(env: SkyEnv): number {
  let sum = 0;
  for (const dir of SKY_SAMPLES) sum += LUM(skyRadiance(dir, env));
  return sum / SKY_SAMPLES.length;
}

export function createSunEnv(THREE: typeof ThreeNamespace): SunEnvRefs {
  return {
    sunDir: { value: new THREE.Vector3(0.35, 0.8, 0.45).normalize() },
    sunColor: { value: new THREE.Color(1.0, 0.95, 0.85) },
    skyColor: { value: new THREE.Color(0.3, 0.42, 0.55) },
    skyHorizon: { value: new THREE.Color(0.62, 0.66, 0.72) },
    skyGround: { value: new THREE.Color(0.13, 0.12, 0.11) },
    skyLumRef: { value: 1 },
    dayLight: { value: 1 },
    powered: { value: 1 },
    glow: { value: 6.0 },
  };
}

export function applySunEnv(
  env: SunEnvRefs,
  timeOfDay: number,
  powered: boolean | undefined,
) {
  const t = timeOfDay;
  const alt = sunAltitude(t);
  const day = dayFactor(t);
  const azRad = ((t / 24) * 360 + 180) * (Math.PI / 180);
  const el = Math.max(alt, -0.45);
  env.sunDir.value
    .set(
      Math.cos(el) * Math.sin(azRad),
      Math.sin(el),
      Math.cos(el) * Math.cos(azRad),
    )
    .normalize();
  // 太阳色：地平线橙 → 正午白 / 夜间月光蓝；天空：day→dusk→night 三段
  const warm = Math.min(1, Math.max(0, alt / 0.32));
  if (alt >= 0) {
    env.sunColor.value.setRGB(1, 0.55 + 0.42 * warm, 0.28 + 0.62 * warm);
  } else {
    env.sunColor.value.setRGB(0.14, 0.17, 0.26); // 月光
  }
  const mix3 = (
    night: [number, number, number],
    dusk: [number, number, number],
    dayv: [number, number, number],
  ): [number, number, number] => [
    night[0] + (dusk[0] + (dayv[0] - dusk[0]) * warm - night[0]) * day,
    night[1] + (dusk[1] + (dayv[1] - dusk[1]) * warm - night[1]) * day,
    night[2] + (dusk[2] + (dayv[2] - dusk[2]) * warm - night[2]) * day,
  ];
  // 天顶：day→dusk→night 三段（原口径）
  env.skyColor.value.setRGB(
    ...mix3([0.016, 0.022, 0.05], [0.24, 0.18, 0.2], [0.3, 0.42, 0.55]),
  );
  // 地平线：低仰角散射更强，白天偏亮、晨昏偏橙红
  env.skyHorizon.value.setRGB(
    ...mix3([0.02, 0.028, 0.06], [0.62, 0.34, 0.22], [0.72, 0.78, 0.84]),
  );
  // 地面反照：粗糙地面的向上散射，白天暖灰、夜间近黑
  env.skyGround.value.setRGB(
    ...mix3([0.008, 0.008, 0.012], [0.1, 0.085, 0.07], [0.17, 0.16, 0.14]),
  );
  // 归一化基准（均值 1 因子）——env 变了必须同步重算，否则间接光整体漂移
  env.skyLumRef.value = Math.max(1e-3, averageSkyLuminance(env));
  env.dayLight.value = day;
  env.powered.value = powered === false ? 0 : 1;
  // 源码 interiorMap.a×16 为 HDR；观察器无 tonemap，白天压 2.5 / 夜间放开 16
  env.glow.value = 2.5 + (16 - 2.5) * (1 - day);
}

/** slot0 参数表 f32 → DataTexture（cols×4 RGBA Float，texelFetch 寻址）。 */
export function buildParamsTexture(
  THREE: typeof ThreeNamespace,
  material: LotMaterial,
): ThreeNamespace.DataTexture | null {
  if (!material.paramsF32 || material.paramCols === 0) return null;
  const texture = new THREE.DataTexture(
    material.paramsF32,
    material.paramCols,
    4,
    THREE.RGBAFormat,
    THREE.FloatType,
  );
  texture.minFilter = THREE.NearestFilter;
  texture.magFilter = THREE.NearestFilter;
  texture.generateMipmaps = false;
  texture.needsUpdate = true;
  return texture;
}

/** 一次 rebuild 的 tint 贴图解析结果（每材质一组）。 */
export interface TintTextureSet {
  tintTex: ThreeNamespace.Texture | null;
  paletteTex: ThreeNamespace.Texture | null;
  normalTex: ThreeNamespace.Texture | null;
  shaderTex: ThreeNamespace.Texture | null;
  interiorTex: ThreeNamespace.Texture | null;
  paramsTex: ThreeNamespace.DataTexture | null;
  paramCols: number;
}

/**
 * 预载全部材质的 tint/palette/normal/shader/interior/params 贴图。
 * uvKind=2（facade tint 着色器）：贴图**预加载完成**后才建材质——
 * 否则首次编译时 uniform 为 null，采样 alpha=0 → 全部 discard（模型隐形）。
 */
async function loadTintTextures(
  THREE: typeof ThreeNamespace,
  materials: LotMaterial[],
  /** 硬件最大各向异性过滤级别（掠射角墙面靠它保清晰度）。 */
  maxAnisotropy = 1,
  /** blob URL 收集（由资产缓存持有，跨 rebuild 存活，释放时统一回收）。 */
  cacheUrls: string[],
): Promise<TintTextureSet[]> {
  const loader = new THREE.TextureLoader();
  // seamless（默认 true）：tint/normal/shader 都是「fract → 图集区域」平铺采样。
  // 禁 mips 是接缝修复的关键——fract 在平铺边界的 UV 跳变会让硬件在 2×2
  // quad 上算出爆炸级导数 → mip 级别错乱 → 每条平铺边界一道 1px 亮线
  // （各向异性还会放大）。引擎用 tex2Dgrad 显式给梯度故无此问题；我们
  // 以近距离检视为主，牺牲远景 shimmer 换无接缝。
  const loadTex = (bytes: Uint8Array<ArrayBuffer>, seamless = true) =>
    new Promise<ThreeNamespace.Texture>((resolve, reject) => {
      const url = pngBlobUrl(bytes);
      // blob URL 由材质资产缓存持有（见 getRefinedMaterialAssets），
      // 不进逐 rebuild 的 textureUrls——缓存命中的贴图要跨 rebuild 存活。
      cacheUrls.push(url);
      loader.load(
        url,
        (texture) => {
          texture.wrapS = THREE.ClampToEdgeWrapping;
          texture.wrapT = THREE.ClampToEdgeWrapping;
          // tint/palette/normal 按后端 bake 同坐标系采样（原始 UV，行 0 =
          // PNG 首行），不做 three 默认的 flipY 翻转。
          texture.flipY = false;
          if (seamless) {
            texture.generateMipmaps = false;
            texture.minFilter = THREE.LinearFilter;
          }
          // 引擎 `tex2Dgrad` + 各向异性采样；仅对保留 mip 的贴图（interior）有意义。
          texture.anisotropy = maxAnisotropy;
          resolve(texture);
        },
        undefined,
        reject,
      );
    });
  return Promise.all(
    materials.map(async (material) => {
      // 逐材质内 5 张 PNG 并行解码（此前串行 await，首载成本 = 各张之和）。
      const [tintTex, paletteTex, normalTex, shaderTex, interiorTex] =
        await Promise.all([
          // tint 的 rg 是调色板坐标（索引数据，非颜色）、a 是 0/1 镜像覆盖：
          // 线性插值会把相邻条目混合成无意义的中间索引——图案边界上的
          // 门窗被"平均"成平墙条目（2026-09-19 消防局 0x4DE9912B 与
          // 0xF8F776BF 两例：padding 均为"禁用 Top"量级，门窗全在 Base 层，
          // 缺失形态与此完全吻合）。与 palette 同理必须点采样。
          material.tintPng
            ? loadTex(material.tintPng).then((t) => {
                t.minFilter = THREE.NearestFilter;
                t.magFilter = THREE.NearestFilter;
                return t;
              })
            : Promise.resolve(null),
          // 调色板 512×16 = 256 列 × 7 行、**每采样点 2×2 像素**，着色器还会加
          // (1/1024,1/32) 把它居中——正是为点采样设计的；线性滤波会把相邻
          // 调色板条目互相抹开。
          material.palettePng
            ? loadTex(material.palettePng).then((t) => {
                t.minFilter = THREE.NearestFilter;
                t.magFilter = THREE.NearestFilter;
                t.generateMipmaps = false;
                return t;
              })
            : Promise.resolve(null),
          material.normalPng ? loadTex(material.normalPng) : Promise.resolve(null),
          material.shaderPng ? loadTex(material.shaderPng) : Promise.resolve(null),
          material.interiorPng
            ? loadTex(material.interiorPng, false).then((t) => {
                // 游戏 interiorMapSampler 为 REPEAT 包装：房间选择偏移（可能为整数倍
                // scale）依赖回绕取样；ClampToEdge 会把越界采样钳成边缘纯色（绿/紫块）
                t.wrapS = THREE.RepeatWrapping;
                t.wrapT = THREE.RepeatWrapping;
                return t;
              })
            : Promise.resolve(null),
        ]);
      return {
        tintTex,
        paletteTex,
        normalTex,
        shaderTex,
        interiorTex,
        // reliefPng（slot5 alpha）高度通道语义未确证（疑为灯亮同源），视差
        // 已回滚——不加载、不上传 GPU。见 Top 层块内回滚记录。
        paramsTex: buildParamsTexture(THREE, material),
        paramCols: material.paramCols,
      };
    }),
  );
}

/** 每 mesh 材质组的延迟绑定贴图集（base/normal/roughness/AO）。 */
export interface MaterialMapSet {
  map: ThreeNamespace.Texture | null;
  normalMap: ThreeNamespace.Texture | null;
  roughnessMap: ThreeNamespace.Texture | null;
  aoMap: ThreeNamespace.Texture | null;
}

interface RefinedMaterialAssetCache {
  payload: LotModelPayload;
  tintSets: TintTextureSet[];
  deferredMaps: MaterialMapSet[];
  /** 缓存持有的 blob URL（释放时统一 revoke）。 */
  urls: string[];
}

/**
 * 材质资产缓存（payload 级）：tint 链 5-6 张/材质 + deferred 链 4 张/材质的
 * PNG 解码与 GPU 上传是 rebuild 最贵的重复功——renderMode/grouping 变化的
 * 全量重建里它们完全不变，命中缓存即零解码。blob URL 由缓存持有，不进
 * 逐 rebuild 的 textureUrls 回收；payload 更换/视口销毁时显式释放。
 */
let materialAssetCache: RefinedMaterialAssetCache | null = null;

/** 释放材质资产缓存（贴图 dispose + blob URL revoke）。 */
export function releaseRefinedMaterialCache(): void {
  const cache = materialAssetCache;
  if (!cache) return;
  materialAssetCache = null;
  for (const url of cache.urls) URL.revokeObjectURL(url);
  for (const set of cache.tintSets) {
    set.tintTex?.dispose();
    set.paletteTex?.dispose();
    set.normalTex?.dispose();
    set.shaderTex?.dispose();
    set.interiorTex?.dispose();
    set.paramsTex?.dispose();
  }
  for (const set of cache.deferredMaps) {
    set.map?.dispose();
    set.normalMap?.dispose();
    set.roughnessMap?.dispose();
    set.aoMap?.dispose();
  }
}

/** 取（或构建）payload 的 tint 链贴图集。命中缓存 = 零解码零上传。 */
export async function getTintTextures(
  THREE: typeof ThreeNamespace,
  payload: LotModelPayload,
  maxAnisotropy: number,
): Promise<TintTextureSet[]> {
  if (!materialAssetCache || materialAssetCache.payload !== payload) {
    releaseRefinedMaterialCache();
    materialAssetCache = {
      payload,
      tintSets: [],
      deferredMaps: [],
      urls: [],
    };
  }
  if (materialAssetCache.tintSets.length === 0) {
    materialAssetCache.tintSets = await loadTintTextures(
      THREE,
      payload.materials ?? [],
      maxAnisotropy,
      materialAssetCache.urls,
    );
  }
  return materialAssetCache.tintSets;
}

/** 取（或构建）payload 的 deferred 链贴图集（await 全部解码完成）。 */
export async function getDeferredMaps(
  THREE: typeof ThreeNamespace,
  payload: LotModelPayload,
  maxAnisotropy: number,
): Promise<MaterialMapSet[]> {
  if (!materialAssetCache || materialAssetCache.payload !== payload) {
    releaseRefinedMaterialCache();
    materialAssetCache = {
      payload,
      tintSets: [],
      deferredMaps: [],
      urls: [],
    };
  }
  if (materialAssetCache.deferredMaps.length === 0) {
    const cache = materialAssetCache;
    const loader = new THREE.TextureLoader();
    const load = async (
      bytes: Uint8Array<ArrayBuffer> | null,
      srgb: boolean,
    ): Promise<ThreeNamespace.Texture | null> => {
      if (!bytes) return null;
      const url = pngBlobUrl(bytes);
      cache.urls.push(url);
      const texture = await loader.loadAsync(url);
      texture.anisotropy = maxAnisotropy;
      if (srgb) texture.colorSpace = THREE.SRGBColorSpace;
      return texture;
    };
    cache.deferredMaps = await Promise.all(
      (payload.materials ?? []).map(async (material) => {
        const [map, normalMap, roughnessMap, aoMap] = await Promise.all([
          load(material.baseColorPng, true),
          load(material.normalPng, false),
          load(material.roughnessPng, false),
          load(material.aoPng, false),
        ]);
        return { map, normalMap, roughnessMap, aoMap };
      }),
    );
  }
  return materialAssetCache.deferredMaps;
}

/** 把 deferred 贴图绑定到逐 mesh 材质组（同步，贴图已就绪）。 */
export function applyDeferredMaterialMaps(
  maps: MaterialMapSet[],
  materialGroups: ThreeNamespace.MeshStandardMaterial[][],
) {
  maps.forEach((set, materialIndex) => {
    const group = materialGroups[materialIndex] ?? [];
    if (!group.length) return;
    if (set.map) {
      for (const refined of group) {
        refined.map = set.map;
        refined.needsUpdate = true;
      }
    }
    if (set.normalMap) {
      for (const refined of group) {
        refined.normalMap = set.normalMap;
        refined.needsUpdate = true;
      }
    }
    if (set.roughnessMap) {
      for (const refined of group) {
        refined.roughnessMap = set.roughnessMap;
        refined.roughness = 1;
        refined.needsUpdate = true;
      }
    }
    if (set.aoMap) {
      for (const refined of group) {
        refined.aoMap = set.aoMap;
        refined.needsUpdate = true;
      }
    }
  });
}

/**
 * tint 着色器注入：逐像素复刻 building4 链（§27/§28 源码逐字）。
 * TEXCOORD_2 = facade 世界投影 UV（Float4.xy），TEXCOORD_1.xy = materialIndex
 * /255 + 内景随机种子。fragment：baseUv = frac(vTintUv)*regionXform.xy +
 * regionXform.zw → tint 查表 → palette 查色（色行+末行 surface 行）×(tint.b*2)，
 * A<0.5 镂空 discard；法线图同 UV 重采样。TBN 直接用 GLB 导出的 TANGENT
 * （<normal_fragment_begin> 的 tbn；= 引擎 ApplyNormalMap(vn, tangent, nmap)
 * 的 ds/dt 轴），仅缺切线时才退回导数拟合。
 * 5a 材质质感：shaderMap 通道×2=specStrength（uSpecG 切 G 数据/B 源码）、
 * palette 色 a³×2048+1=specE、surface 行 a=reflectance、gloss、AO=normalMap.a；
 * SimCityLighting 太阳 Blinn-Phong-Schlick 高光 + EnvLighting 常数天空近似。
 * 5b 假内景（ClipAndReliefMapPS/InteriorMapPS）：shaderMap.a=窗洞混合因子，
 * uv*roomInvSize 栅格化逐窗格 FastNoise 选房（4 变体×象限），interiorMap()
 * 盒体投影进 slot5 房间图集（interiorScale/Offset=row0.zw，roomInvSize=
 * row3.zw），夜灯=interiorMap.a×uInteriorGlow。
 * 30.2 Top 层双采样（TEXCOORD_3 = Float4.zw = uv2）：topUv =
 * frac(vTopUv)*regionXform2(row2)+offset，facadeTint.a@topUv = 窗户 motif
 * 覆盖率，shaderMap/normalMap/palette(palU2 列)/surface/亮度全部按其 lerp
 * ——公寓楼窗标记只在 Top 域（facade_survey 普查 86%），Base-only 采样
 * 会导致窗户全墙化。
 * 偏离源码处（均文档化）：①specularity 取 G 通道（资产实证）；②内景自发光
 * 16→uInteriorGlow 可调（无 HDR tonemap）；③interiorThresholds 用常数四分位
 * （引擎值未知）；④eyeDir 用对象空间近似切线空间。
 * 【2026-09-26 移除】下向面豁免镂空（曾作观察器缓解）——它把桁架等真洞
 * 渲染成白面片（用户实证），回归引擎无条件 clip 口径，见 map_fragment 块。
 */
export function attachTintShader(
  material: ThreeNamespace.MeshStandardMaterial,
  uniforms: {
    tintMap: { value: ThreeNamespace.Texture };
    paletteMap: { value: ThreeNamespace.Texture };
    shaderMapMap: { value: ThreeNamespace.Texture | null };
    interiorMapMap: { value: ThreeNamespace.Texture | null };
    paramsMap: { value: ThreeNamespace.Texture | null };
    uParamCols: { value: number };
    uSunDir: { value: ThreeNamespace.Vector3 };
    uSunColor: { value: ThreeNamespace.Color };
    uSkyColor: { value: ThreeNamespace.Color };
    /** 解析天空三段（天顶/地平/地面）+ 球面均值亮度基准（见 skyRadiance）。 */
    uSkyHorizon: { value: ThreeNamespace.Color };
    uSkyGround: { value: ThreeNamespace.Color };
    uSkyLumRef: { value: number };
    uSpecMode: { value: number };
    uInteriorGlow: { value: number };
    /** 5d 日/夜：白昼因子 0..1（夜间环境/漫反射压暗、内景环境光） */
    uDayLight: { value: number };
    /** 5d 供电：0 = 内景自发光全灭（源码 interiorThresholds.z） */
    uPowered: { value: number };
    /** tint 图集半 texel（1/宽, 1/高）：平铺区域边缘的线性滤波内缩量。 */
    uTintTexel: { value: ThreeNamespace.Vector2 };
  },
  paramsReady: boolean,
  shaderMapReady: boolean,
  interiorReady: boolean,
) {
  // 注意：three 默认编译为 GLSL ES 1.00——texelFetch/ivec2 不可用，
  // 参数表用 texture2D + 预计算 V 寻址（Nearest 采样取整行）。
  material.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, uniforms);
    shader.vertexShader = shader.vertexShader
      .replace(
        "#include <common>",
        `#include <common>
attribute vec4 uv1;
attribute vec2 uv2;
attribute vec2 uv3;
uniform float uParamCols;
#ifdef TINT_PARAMS
uniform sampler2D paramsMap;
#endif
varying vec2 vTintUv;
varying vec2 vTopUv;
varying vec4 vXform;
varying vec4 vXform2;
varying vec4 vPalOrigin;
varying vec4 vRoom;
varying float vObjUp;
varying float vSeed;
varying vec3 vObjEyeDir;
varying vec3 vModelPos;`,
      )
      .replace(
        "#include <uv_vertex>",
        `#include <uv_vertex>
vTintUv = uv2;
vTopUv = uv3;
// 参数表按顶点取行（引擎 building4DefaultVS 同款数据流：VS 查表 →
// regionXform 作为 varying 插值）。此前在片元里用插值列号 vMatU 查表：
// 跨列三角形的列号在边界间连续扫过一连串无关列，窗扇半边被换成素墙
// 区域（消防局中窗右半变砖墙，2026-09-19 实测）。列号取整后 +0.5 对齐
// texel 中心，Nearest 采样行 V 与片元版一致。
#ifdef TINT_PARAMS
float scMatCol = floor(uv1.x * 255.0 + 0.5);
vec2 scMatC = vec2((scMatCol + 0.5) / uParamCols, 0.0);
vPalOrigin = texture2D(paramsMap, scMatC + vec2(0.0, 0.125));
vXform = texture2D(paramsMap, scMatC + vec2(0.0, 0.375));
vXform2 = texture2D(paramsMap, scMatC + vec2(0.0, 0.625));
vRoom = texture2D(paramsMap, scMatC + vec2(0.0, 0.875));
#else
vPalOrigin = vec4(0.0);
vXform = vec4(1.0, 1.0, 0.0, 0.0);
vXform2 = vec4(0.0);
vRoom = vec4(0.0);
#endif`,
      )
      .replace(
        "#include <beginnormal_vertex>",
        `#include <beginnormal_vertex>
vObjUp = normalize(objectNormal).z;
vSeed = uv1.y;
vModelPos = (modelMatrix * vec4(0.0, 0.0, 0.0, 1.0)).xyz;
{
  vec4 scMv = modelViewMatrix * vec4(position, 1.0);
  mat3 scNm = normalMatrix;
  mat3 scNmT = mat3(scNm[0][0], scNm[1][0], scNm[2][0],
                    scNm[0][1], scNm[1][1], scNm[2][1],
                    scNm[0][2], scNm[1][2], scNm[2][2]);
  vObjEyeDir = normalize(scNmT * normalize(-scMv.xyz));
}`,
      );
    shader.fragmentShader = shader.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
varying vec2 vTintUv;
varying vec2 vTopUv;
varying vec4 vXform;
varying vec4 vXform2;
varying vec4 vPalOrigin;
varying vec4 vRoom;
varying float vObjUp;
varying float vSeed;
varying vec3 vObjEyeDir;
varying vec3 vModelPos;
uniform sampler2D tintMap;
uniform sampler2D paletteMap;
uniform vec3 uSunDir;
uniform vec3 uSunColor;
uniform vec3 uSkyColor;
uniform vec3 uSkyHorizon;
uniform vec3 uSkyGround;
uniform float uSkyLumRef;
// 世界方向（view→world；viewMatrix 是正交旋转，转置即逆）
vec3 scToWorldDir(vec3 v) {
  return vec3(dot(v, viewMatrix[0].xyz), dot(v, viewMatrix[1].xyz), dot(v, viewMatrix[2].xyz));
}
// 解析天空辐照（引擎 SkyColorConv 的近似）：天顶↔地平三段 + 太阳邻近辉光
// + 地平线以下的地面反照。**必须与 refinedRender.ts 的 skyRadiance() 逐式一致**
// ——后者用同一式子在球面采样求均值，标定 uSkyLumRef。
vec3 scSkyRadiance(vec3 d) {
  float up = clamp(d.z, -1.0, 1.0);
  vec3 sky = mix(uSkyHorizon, uSkyColor, pow(saturate(up), 0.45));
  sky += uSunColor * pow(saturate(dot(d, uSunDir)), 8.0) * 0.30;
  return mix(uSkyGround, sky, smoothstep(-0.15, 0.05, up));
}
uniform float uSpecMode;
uniform float uInteriorGlow;
uniform float uDayLight;
uniform float uPowered;
uniform vec2 uTintTexel;
#ifdef TINT_SHADERMAP
uniform sampler2D shaderMapMap;
#endif
#ifdef TINT_INTERIOR
uniform sampler2D interiorMapMap;
// ClipAndReliefMapPS：盒体裁剪 + 透视投影（kInvDepth=0.5/kBackSize=0.5/kDilation=0.9）
vec2 scInteriorMap(vec3 eye, vec2 tc, float invMapDepth, float backSize, float dilation) {
  vec3 eyeDir = eye;
  eyeDir.z *= invMapDepth;
  vec3 pos = vec3(tc, 0.0) * -2.0 + 1.0;
  pos.z -= 1.0;
  vec3 k = (sign(eyeDir) - pos) / eyeDir;
  float t = min(k.x, min(k.y, k.z));
  vec3 target = pos + t * eyeDir;
  target.xy *= mix(dilation, backSize, target.z);
  return target.xy * -0.5 + 0.5;
}
// 源码 FastNoise 逐字
float scFastNoise(vec3 seed) {
  seed *= vec3(78.233, 12.9898, 43758.5453);
  seed += vec3(0.819 * 78.233, 0.819 * 12.9898, 0.819 * 43758.5453);
  return fract(seed.z * fract(seed.x * fract(seed.y)));
}
#endif`,
      )
      .replace(
        "#include <map_fragment>",
        `#include <map_fragment>
        // 参数表行 = VS 按顶点查表后的 varying（引擎同款数据流），
        // 跨列三角形平滑插值区域变换而非扫过无关列。
        vec4 xform = vXform;
        vec4 xform2 = vXform2; // row2=regionXform2(Top 层)
        vec4 palOrigin = vPalOrigin;
        vec4 scRoom = vRoom; // row3=(tilePadding.xy, roomInvSize.zw)
        // 半 texel 内缩：tint 是图集，fract=0/1 处的线性滤波核会读到相邻
        // 区域内容（Base 层此前没有 padding 保护——接缝的第二个成因）。
        vec2 tUv = fract(vTintUv) * max(xform.xy - uTintTexel, vec2(0.0)) + xform.zw + uTintTexel * 0.5;
        vec4 tintValues = texture2D(tintMap, tUv);
        // 30.2 Top 层（relief_tc 域，uv2×regionXform2）：窗户 motif 所在。
        // 源码（cpp frac 变体定谳）：tilePadding=row3.xy，且
        // reliefSrc = frac(uv2)·(1+padding) − padding/2，越出 [0,1] →
        // outsideTile>0 → facadeTint.a 强制 0（退回 Base 层）。玻璃幕墙楼
        // padding 高达 ~8e4（数值即语义：整体禁用 Top），公寓楼 ~(0.125,0)。
        // 【2026-09-19 回滚记录】曾尝试把 >1 的 padding 分量按 0 处理
        // （理由：消防局 Top 区域看起来承载门窗图案），结果整个立面的
        // facadeTint.a 全程生效、窗户图案漫延全墙——渲染彻底错乱，已回滚。
        // 大数值 padding 的刀带效应（reliefSrc 几乎处处越界）= 引擎语义的
        // 「Top 关闭」，原判读正确。消防局窗户实际来自 Base 层的逐顶点
        // 选列（Color 通道 D3DCOLOR.G），col 0 只是素墙区域之一。
        vec2 topUv = vec2(0.0);
        float scFacade = 0.0;
        vec4 facadeTintValues = vec4(0.0);
        if (xform2.x > 0.0 && xform2.y > 0.0) {
          vec2 scPad = scRoom.xy;
          vec2 reliefSrc = fract(vTopUv) * (1.0 + scPad) - scPad * 0.5;
          #ifdef TINT_RELIEF
          // 【2026-09-20 回滚】曾按标准 relief mapping 补写视差（用 reliefPng
          // alpha 作高度），实证高度通道语义不可靠：reliefPng 是 slot5 alpha
          // 的另一份导出——与「逐窗灯亮通道」同源，拿灯亮当深度会把窗户
          // motif 拖到全墙/屋顶任意位置并随视角闪烁（多楼实证）。游戏本编译
          // 版 reliefMap() 本就是恒等（被裁剪），回滚后与游戏行为一致。
          // 重启前提：先从 Wwise/DXT5 原始数据确证真实高度通道。
          #endif
          float outsideTile =
            max(-reliefSrc.x, 0.0) + max(-reliefSrc.y, 0.0) +
            max(reliefSrc.x - 1.0, 0.0) + max(reliefSrc.y - 1.0, 0.0);
          topUv = clamp(reliefSrc, 0.0, 1.0) * max(xform2.xy - uTintTexel, vec2(0.0)) + xform2.zw + uTintTexel * 0.5;
          facadeTintValues = texture2D(tintMap, topUv);
          scFacade = (outsideTile > 0.0) ? 0.0 : facadeTintValues.a;
        }
        vec2 scSubTop = facadeTintValues.rg * vec2(1.0 / 512.0, 1.0 / 16.0) + vec2(1.0 / 1024.0, 1.0 / 32.0);
        vec2 scSub = tintValues.rg * vec2(1.0 / 512.0, 1.0 / 16.0) + vec2(1.0 / 1024.0, 1.0 / 32.0);
        vec4 scPalColor = vec4(1.0);
        float scTintMul = tintValues.b * 2.0;
        vec4 scShaderMap = vec4(1.0);
        if (tintValues.a < 0.5) {
          // 引擎 building4Clip 的镂空是**无条件 clip**（不按朝向豁免）。
          // 此前的「下向面豁免」观察器缓解（地板底面继承镂空模板、从下仰视
          // 见穿透洞）会把桁架等**真洞**渲染成白色面片——2026-09-26 用户
          // 实证（铁梯桁架三角孔白色、同资产水平面洞正常），移除豁免回归
          // 引擎口径；仰视穿透属引擎本征行为。
          discard;
        } else {
          // 源码 lerp(tintBase@palU, tintTop@palU2, facadeTint.a)：Top 层查
          // 调色板第二列（row0.y = palU2），亮度/子采样坐标同样取 Top 值
          vec4 scPalBase = texture2D(paletteMap, vec2(palOrigin.x + scSub.x, scSub.y));
          vec4 scPalTop = texture2D(paletteMap, vec2(palOrigin.y + scSubTop.x, scSubTop.y));
          scPalColor = mix(scPalBase, scPalTop, scFacade);
          scTintMul = mix(tintValues.b, facadeTintValues.b, scFacade) * 2.0;
          diffuseColor.rgb *= scPalColor.rgb * scTintMul;
          #ifdef USE_NORMALMAP
          // artistAO = normalMapSampled.a（Base/Top 双采样 lerp）
          float scAo = texture2D(normalMap, tUv).a;
          if (scFacade > 0.001) scAo = mix(scAo, texture2D(normalMap, topUv).a, scFacade);
          diffuseColor.rgb *= scAo;
          #endif
        }
        #ifdef TINT_SHADERMAP
        {
          vec4 smBase = texture2D(shaderMapMap, tUv);
          scShaderMap = smBase;
          if (scFacade > 0.001) {
            scShaderMap = mix(smBase, texture2D(shaderMapMap, topUv), scFacade);
          }
        }
        #endif
        // 5a spec 四标量（building4DeferredPS；色/表面行均 ×tintMul）。
        // specStrength 通道（uSpecMode）：0=自动逐像素（资产实证——墙面
        // specularity 在 .g、窗玻璃在 .b，与窗洞掩码 a 互补，如金样本墙面
        // G=168/窗洞 B=101.5）；1=强制 G；2=强制 B（源码字面）。
        float scSpecRaw = scShaderMap.b;
        if (uSpecMode < 0.5) {
          scSpecRaw = mix(scShaderMap.b, scShaderMap.g, step(0.5, scShaderMap.a));
        } else if (uSpecMode < 1.5) {
          scSpecRaw = scShaderMap.g;
        }
        float scSpecStrength = scSpecRaw * 2.0;
        float scSpecA = scPalColor.a * scTintMul;
        float scSpecE = scSpecA * scSpecA * scSpecA * 2048.0 + 1.0;
        float scGloss = clamp(scSpecA * scSpecStrength, 0.0, 1.0);
        vec4 scSurfaceBase = texture2D(paletteMap, vec2(palOrigin.x + scSub.x, 0.875 + scSub.y)); // kSurfacePalV
        vec4 scSurfaceTop = texture2D(paletteMap, vec2(palOrigin.y + scSubTop.x, 0.875 + scSubTop.y));
        vec4 scSurface = mix(scSurfaceBase, scSurfaceTop, scFacade);
        float scReflectance = scSurface.a * scTintMul;
        // 5b 假内景（ClipAndReliefMapPS + InteriorMapPS）：shaderMap.a = 窗洞
        // 混合因子（1=外观，0=透内景）。逐窗格栅格化 + FastNoise 选房
        // （4 变体 × 象限单元格），盒体投影进 slot5 房间图集。
        vec3 scInterior = vec3(0.0);
        float scOpacity = 1.0;
        #ifdef TINT_INTERIOR
        scOpacity = scShaderMap.a;
        if (scOpacity < 0.999) {
          float scSeedEff = vSeed;
          if (vSeed < 0.5) {
            // 源码：种子 <0.5 时按模型位置 munge
            vec2 scQ = floor(vModelPos.xy * 2.0) + floor(vModelPos.zz * 2.0);
            scSeedEff = scFastNoise(vec3(scQ, vSeed));
          }
          // 源码逐字：interiorUv = uv * regionXform.xy * interiorRoomInvSize。
          // roomInvSize = row3.zw、tilePadding = row3.xy（cpp frac 变体定谳；
          // 玻璃楼 padding~8e4 禁 Top / 公寓楼 (0.125,0) 两样本互证）。
          vec2 scInteriorUv = vTintUv * xform.xy * scRoom.zw;
          vec2 scInteriorElem = floor(scInteriorUv);
          vec2 scInteriorSrcUv = fract(scInteriorUv);
          // eyeDir 切线空间化（源码 eyeDir = -mul(tangentSpace, viewPos)，t1 插值）：
          // 用 vTintUv 的屏幕导数重建与立面 UV 轴对齐的切线架，把视线向量
          // （fragment→相机，view space）变换进 (u, v, normal) 基。盒体裁剪+
          // 透视投影（前 0.9/后 0.5）只有在该空间才成立。
          vec3 scPosV = -vViewPosition;
          vec3 scDpx = dFdx(scPosV);
          vec3 scDpy = dFdy(scPosV);
          vec2 scDuvx = dFdx(vTintUv);
          vec2 scDuvy = dFdy(vTintUv);
          float scDet = scDuvx.x * scDuvy.y - scDuvy.x * scDuvx.y;
          vec3 scTanU = vec3(1.0, 0.0, 0.0);
          vec3 scTanV = vec3(0.0, 1.0, 0.0);
          if (abs(scDet) > 1e-10) {
            scTanU = normalize((scDpx * scDuvy.y - scDpy * scDuvx.y) / scDet);
            scTanV = normalize((scDpy * scDuvx.x - scDpx * scDuvy.x) / scDet);
          }
          vec3 scEyeVecV = normalize(vViewPosition);
          vec3 scGeomN = normalize(vNormal);
          vec3 scEyeTan = vec3(
            dot(scEyeVecV, scTanU),
            dot(scEyeVecV, scTanV),
            dot(scEyeVecV, scGeomN)
          );
          vec2 scResultTc = scInteriorMap(scEyeTan * scRoom.zwz, scInteriorSrcUv, 0.5, 0.5, 0.9);
          vec2 scInteriorTc = scResultTc * vec2(palOrigin.z) + vec2(0.0, palOrigin.w);
          float scRoomId = scFastNoise(vec3(scInteriorElem, scSeedEff));
          float scRoomVariation = floor(scRoomId * 4.0);
          // interiorThresholds 引擎值未知，v1 用四分位（0.25/0.5/0.75）
          vec4 scEdge = vec4(step(vec3(0.25, 0.5, 0.75), vec3(scRoomId)), scRoomVariation * 4.0);
          scInteriorTc.x += dot(scEdge, vec4(1.0)) * palOrigin.z;
          vec4 scRoomTex = texture2D(interiorMapMap, scInteriorTc);
          // 内景照明：房间环境光随昼夜（夜间仅微光）+ 灯亮 a×glow（HDR×16
          // 的 tonemap 近似，白天压 2.5）×供电（断电全灭，源码 .z hack）
          float scSelfLight = scRoomTex.a * uInteriorGlow * uPowered;
          // 源码夜间项 = shColorDiff（夜空 SH 非零 + 城市光，非纯黑）；
          // 常数 0.12 灰是此前无天空近似时的占位——改为月亮底光 + 天空项，
          // 下限 (0.10,0.12,0.18) 保证房间结构在深夜可辨。
          vec3 scNightAmb = max(
            scSkyRadiance(normalize(vec3(0.35, 0.35, 1.0))) * 0.9,
            vec3(0.10, 0.12, 0.18)
          );
          vec3 scAmbient = mix(scNightAmb, vec3(1.0), uDayLight);
          scInterior = scRoomTex.rgb * (scAmbient + scSelfLight);
        }
        #endif
        diffuseColor.rgb = mix(scInterior, diffuseColor.rgb, scOpacity);`,
      )
      .replace(
        "#include <normal_fragment_maps>",
        `#include <normal_fragment_maps>
        #ifdef USE_NORMALMAP_TANGENTSPACE
        {
          // 用 three 建好的 tbn（<normal_fragment_begin> 里）：GLB 带 TANGENT
          // 时 = (vTangent, vBitangent, normal)，与引擎 building4DefaultPS 的
          // ApplyNormalMap(vn, tangent, nmap) 同帧；缺切线时才退化为导数拟合。
          // 此前这里自建了一个 tbn 局部遮蔽它，等于永远走导数路径。
          // 法线与反照率共用 tUv/topUv（同区域、同内缩，像素对齐）。
          vec3 mapN = texture2D( normalMap, tUv ).xyz * 2.0 - 1.0;
          // Top 层法线（窗框/线脚凹凸）按 facadeTint.a lerp（引擎同用一个 TBN）
          if (scFacade > 0.001) {
            vec3 nTop = texture2D( normalMap, topUv ).xyz * 2.0 - 1.0;
            mapN = mix(mapN, nTop, scFacade);
          }
          mapN.xy *= normalScale;
          normal = normalize( tbn * mapN );
        }
        #endif`,
      )
      .replace(
        "#include <lights_fragment_end>",
        `#include <lights_fragment_end>
        // 游戏 SimCityLighting（building4DeferredPS）：太阳 Blinn-Phong-Schlick 高光
        // + EnvLighting 解析天空。源码半向量 = normalize(lightDir - viewDir)、能量
        // 归一 (specE+2)/8、Schlick exp2(-8.656170·cosLH)。
        {
          vec3 scSunV = normalize((viewMatrix * vec4(uSunDir, 0.0)).xyz);
          vec3 scHalf = normalize(scSunV - normalize(vViewPosition));
          float scNDotH = clamp(dot(normal, scHalf), 0.0, 1.0);
          float scSpec = pow(scNDotH, max(scSpecE, 0.001)) * ((scSpecE + 2.0) / 8.0);
          float scSchlick = scReflectance + (1.0 - scReflectance) * exp2(-8.656170 * clamp(dot(scSunV, scHalf), 0.0, 1.0));
          float scSunMod = clamp(dot(scSunV, normal), 0.0, 1.0);
          reflectedLight.directSpecular += scSpec * scSchlick * scSpecStrength * scSunMod * uSunColor;
          // EnvLighting（源码字面结构）：s = gloss*0.75，采样方向在天线与反射向之间
          // 插值；反射向按源 DefaultPS 的 bentViewDirection 弯折（掠射时加强）。
          vec3 scNormW = scToWorldDir(normalize(normal));
          vec3 scBent = scToWorldDir(normalize(vViewPosition)); // eye→frag（世界）
          scBent.z += 2.0 * saturate(-scBent.z) * (1.0 - saturate(scNormW.z));
          scBent = normalize(scBent);
          float scEnvS = scGloss * 0.75;
          vec3 scEnvDir = normalize(mix(scNormW, reflect(scBent, scNormW), scEnvS));
          vec3 scEnv = scSkyRadiance(scEnvDir);
          reflectedLight.indirectSpecular +=
            scEnv * scEnvS * diffuseColor.rgb * (1.0 - scExempt);
          // 间接漫反射按天空方向重分配（= 源码 EnvLighting 的 SkyColor(sampleDir)）：
          // 用亮度比 scLum/uSkyLumRef 作乘性因子，**球面均值为 1**——只改变各朝向的
          // 环境光分布，不抬整体曝光。此前是常数 AmbientLight，各朝向完全相同（发平）。
          float scLum = dot(scSkyRadiance(scNormW), vec3(0.2126, 0.7152, 0.0722));
          // 0.6 = 强度旋钮（0 退回常数环境光，1 全量）。因子均值恒为 1。
          float scDirFactor = mix(1.0, clamp(scLum / max(uSkyLumRef, 1e-3), 0.25, 2.5), 0.6);
          reflectedLight.indirectDiffuse *= mix(1.0, scDirFactor, 1.0 - scExempt);
          // 5d 夜间：three 侧灯光的漫反射分量随白昼因子压暗（太阳高光/
          // 天空镜面已由 uSunColor/天空三段变暗）
          float scNightDim = mix(0.22, 1.0, uDayLight);
          reflectedLight.directDiffuse *= scNightDim;
          reflectedLight.indirectDiffuse *= scNightDim;
        }`,
      );
  };
  material.customProgramCacheKey = () =>
    `building4-tint${shaderMapReady ? "+sm" : ""}${paramsReady ? "+pm" : ""}${interiorReady ? "+im" : ""}`;
}

/**
 * 为 uvKind=2 mesh 建 tint 材质（含着色器注入与共享环境 uniform）。
 * 返回 [材质, specMode uniform 引用]（引用供热切换收集）。
 */
export function makeTintMaterial(
  THREE: typeof ThreeNamespace,
  tint: TintTextureSet,
  env: SunEnvRefs,
): [ThreeNamespace.MeshStandardMaterial, { value: number }] {
  const tinted = new THREE.MeshStandardMaterial({
    roughness: 0.9,
    metalness: 0,
    side: THREE.DoubleSide,
    normalMap: tint.normalTex,
  });
  // 法线观感强度：引擎 ApplyNormalMap 的倍率未知，资产本身是细腻浮雕
  // （窗框/线脚）；1.0 时几乎不可辨（2026-09-18 用户反馈），1.5 为
  // 可见但不夸张的折中。normalTex 缺失时该值无副作用。
  tinted.normalScale = new THREE.Vector2(1.5, 1.5);
  tinted.defines = { USE_UV: "" };
  if (tint.paramsTex) tinted.defines.TINT_PARAMS = "";
  if (tint.shaderTex) tinted.defines.TINT_SHADERMAP = "";
  const interiorReady = Boolean(
    tint.paramsTex && tint.shaderTex && tint.interiorTex,
  );
  if (interiorReady) tinted.defines.TINT_INTERIOR = "";
  const uSpecGUniform = { value: effectiveSpecMode() };
  // tint 图集半 texel：shader 里的平铺区域边缘内缩量（接缝修复）。
  const tintImage = tint.tintTex?.image as
    { width?: number; height?: number } | undefined;
  const uTintTexel =
    tintImage?.width && tintImage?.height
      ? new THREE.Vector2(0.5 / tintImage.width, 0.5 / tintImage.height)
      : new THREE.Vector2(0, 0);
  attachTintShader(
    tinted,
    {
      tintMap: { value: tint.tintTex! },
      paletteMap: { value: tint.paletteTex! },
      shaderMapMap: { value: tint.shaderTex },
      interiorMapMap: { value: tint.interiorTex },
      paramsMap: { value: tint.paramsTex },
      uParamCols: { value: tint.paramCols },
      // 5a/5d：太阳/天空/昼夜/供电为共享 uniform 实例（applySunEnv 热切换）
      uSunDir: env.sunDir,
      uSunColor: env.sunColor,
      uSkyColor: env.skyColor,
      uSkyHorizon: env.skyHorizon,
      uSkyGround: env.skyGround,
      uSkyLumRef: env.skyLumRef,
      uSpecMode: uSpecGUniform,
      uInteriorGlow: env.glow,
      uDayLight: env.dayLight,
      uPowered: env.powered,
      uTintTexel: { value: uTintTexel },
    },
    Boolean(tint.paramsTex),
    Boolean(tint.shaderTex),
    interiorReady,
  );
  return [tinted, uSpecGUniform];
}

