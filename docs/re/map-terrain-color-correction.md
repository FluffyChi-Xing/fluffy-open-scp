# 区域地形着色勘误（2026-10-08）

本文修正 `map-3d-preview-research.md` 和 `map-ground-coloring-and-playable-region.md`
中把 ED b1 当草量、b2 当材质权重的推断。适用当前 3D 地图预览。

## 根因与证据

本地 dev 数据根：`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/SC/`。

1. `cTerrainExporter/ExportRegionEcoMapsRecursively.c`（0x419FC0）依次把
   soil、forest、waterTable 与 DefaultBlack 传入 CombineAndMask 的 R/G/B/A。
   `ExportColorAffectingEcoMaps.c`（0x419110）互证类型 1/3/2 的名称。
2. `cTerrainExporter/BeginExporting.c` 为生态图分配 `FORMAT_A8R8G8B8`。
   小端文件像素顺序因此是 **b0=地下水、b1=森林、b2=土壤、b3=污染**。
   静态区域导出使用 DefaultBlack，污染通道为零。
3. `cTerrainEcoMapCombiner/ConfigureDefault.c`（0x42D850）确认运行时同样使用
   soilMap / forestMap / waterTableMap / groundPollutionMap 的顺序。
4. 项目已提取的 `tmp/ps_fragments.json` 中，`terrainPS` 的共享 HLSL
   `getGrassAmount` 完整实现为：

   ```hlsl
   ecoMapsOut = tex2D(combinedEcoMaps, heightMapUV).rgba;
   float grassAmount = sqrt(ecoMapsOut.r * ecoMapsOut.b);
   float patchy = tex2D(noiseMap, controlMapUV * GetPatchyNoiseUVScale()).x;
   patchy = patchy * GetPatchyNoiseScale() + GetPatchyNoiseOffset();
   grassAmount = sign(grassAmount) * saturate(grassAmount + patchy);
   float flatness = saturate(terrainNormal.z);
   flatness *= flatness;
   flatness *= flatness;
   flatness *= flatness;
   flatness *= flatness;
   grassAmount *= flatness;
   grassAmount = saturate(grassAmount);
   grassAmount *= 1.0 - controlAmount;
   ```

5. `terrainPS` 以 `max(abs(normal)-0.15,0)` 归一化后混合岩壁与地表，
   并将 forestColorMap 单独混合。没有“海拔超过 950m 自动变雪”的逻辑。
   旧文提及的 `basicGroundColor *= basicGroundColor` 属于 **lot lawn** 路径；
   不能把它再套在已做 sRGB→线性转换的区域地形调色板上。

泰坦峡谷实测（当前名字链：C04182E4 → LittleGorge）：

| 指标 | 数值 |
|---|---:|
| 海平面以上 3.125m 的采样数 | 1,111,224 |
| 地下水 b0 均值（0–255） | 137.121 |
| 森林 b1 均值（0–255） | 13.199 |
| 土壤 b2 均值（0–255） | 125.555 |
| sqrt(soil×water) 均值（0–1，未乘坡度） | 0.429 |

旧实现以森林均值 <20 判整个区域为荒漠，并用 `data.desert ? 0 : ...`
强制清除所有绿地。此地图正好触发该分支；调整绿色色值无法解决此问题。

## 实现范围

- Rust 输出生态 PNG 改为 **R=土壤、G=森林、B=地下水、A=255**。
  alpha 必须保持不透明，避免 canvas 预乘污染数据。前后端需同时更新。
- desert 元数据改从土壤×地下水推导，仅作描述；着色器不再使用全图开关。
- `terrain-material.ts` 独立负责地表材质；`MapViewer3D.vue` 负责网格、相机、
  水面和覆盖物，props 接口仍是 `Region3DData` / `showPlots`。
- 两张无颜色空间转换的 DataTexture 保存生态通道与源分辨率法线。
  逐片元采样不受网格 LOD 降采样影响；法线由高度场中心差分得到。
  Three 为 Y-up，使用 world normal.y 对应 HLSL 的 normal.z。
- 草地、岸边湿沙、岩面分别混合；森林通道只提供远景树冠色斑。
- 延续原有 b0==0 路网近似：游戏导出时控制图会清除森林/地下水通道，
  但自然无地下水区也可能为零，不能将这个判据宣称为精确道路类型数据。

确定复刻的部分是通道语义、草量平方根、法线的 16 次方与三向混合权重。
地表漫反射已替换为 App 包原始 cube 的四个面（见下节）。噪声幅度、树冠斑点、
平铺尺度和光照仍是预览近似；未恢复树模型、完整天气/天空光照和游戏法线纹理。
保持使用 Three 的线性光照与 sRGB 输出，不额外平方。

## 原始地表纹理接入（2026-10-08）

`cMaterialGroundType::Init` 的反编译局部变量名造成了早期检索误判：
名称 FNV-1 哈希是 **instance**，不是 group。实际资源位于
`SimCity_App.package`，type=`03E421ED`、group=`00000000`：

| 用途 | 名称 | instance |
| --- | --- | --- |
| 泥土 | terrain_dirtdetail | 652665DB |
| 草地 | terrain_grassdetail | D85F0A84 |
| 岩壁 | terrain_cliffx | 1549AAB9 |
| 沙滩 | terrain_beach | C7D28B18 |

每个资源为 20 字节头 + 256×256 BGRA 像素。后端从地图包旁的 App 包读取、
转成不透明 PNG，经 `terrainTextures` 返回前端，游戏资产不入库。
缺少旁包或单个资源时仅该面回退到原色板。
前端按 sRGB 漫反射纹理加载，开启重复、mipmap 和各向异性过滤；
草/土/沙世界周期暂定 64m，岩壁 128m，跨区块与 LOD 连续。
这是默认地表材质接入；区域自定义 cube 覆盖、细节法线及原始调参仍未完整复刻。

## 检查与复现

```powershell
cargo run -p sc-properties --example terrain_color_probe -- D:/ea-games/SimCity/SimCityData C04182E4 tmp/terrain-color-titan
cargo test -p sc-properties --lib
node node_modules/vue-tsc/bin/vue-tsc.js -b --noEmit
node node_modules/vite/bin/vite.js build
```

探针将 PNG 与区域元数据输出到指定目录，游戏资产不进入版本库。
本次实际 WebGL 检查：湿润无森林、湿润森林、缺水/缺土、旧 desert 开关、
相机旋转、陡坡六项通过；泰坦峡谷全图/缩放渲染无着色器错误。
截图与像素结果位于 `tmp/terrain-color-titan/`。

用户目视检查后已授权：修复英文地块标签后提交本阶段地图变更。
