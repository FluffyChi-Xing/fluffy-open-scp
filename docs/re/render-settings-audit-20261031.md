# 建筑贴图/反射清晰度：游戏渲染设置对拍

日期：2026-10-31

## 1. 游戏侧设置（反编译证据）

来源：`docs/source-code/SimCity-dev-ida.c`

| 项 | 游戏行为 | 证据 |
| --- | --- | --- |
| 各向异性 | 开关打开时 **MAXANISOTROPY = 8**；MINFILTER=Linear 且 MIPFILTER=Linear 时自动升级为 Anisotropic | :278970-278971（`sEnableAnisotropic ? 8 : 1`）、:292558-292559、:373654-373679 |
| Mip LOD | 默认 `mCurrentMipLodSetting = 0`，由画质属性 `0x02840048` 覆盖；`Raster::SetLOD` 按该值限制 mip | :454732-454736、:456786 |
| MSAA | 后台缓冲多重采样由设备能力申请（`rwg_SetPresent`，ida 358964） | `docs/re/building-facade-engine-analysis.md` §1 |
| 采样 | building4 全部 `tex2Dgrad` 显式导数 + LINEAR + mip | 同上 §2.2 |
| 反射 | `reflectionSampler` 是**具体 cubemap 纹理**（CombinePS `texCUBE`） | `building-facade-engine-analysis.md` §9.4 |

## 2. 当前前端设置

- `WebGLRenderer({ antialias: true, alpha: true, powerPreference: "high-performance" })`
- `setPixelRatio(min(devicePixelRatio, maxPixelRatio=2))`；交互期 1，静止恢复
- 建筑贴图：`LINEAR + mipmap + anisotropy = renderer 上限`
  （`refinedRender.ts` loadTex / getDeferredMaps）
- 采样：`textureGrad` + 未 fract 域显式导数；调色板 POINT；Top/Base 分域导数
- 运行时实测（headless SwiftShader，9B2 小屋）：
  `anisotropy=4`（SwiftShader 上限）、`minFilter=1008 (LinearMipmapLinear)`、
  `generateMipmaps=true`；真实 GPU 上限通常 16
- 建筑材质来源：`model.lotm` 内 PNG 尺寸实测 256×256（normal 512×512），
  与游戏资产同分辨率

## 3. 结论：过滤链没有遗漏，差异在“反射”和资产分辨率

1. 游戏用 8× 各向异性；前端用硬件上限（≥8 时更锐），mip + LINEAR +
   显式导数也已对齐。**贴图过滤参数没有发现缺失**。
2. 建筑 PNG 为 256²（normal 512²）。近距离墙面占屏较大时，256² 本身
   就是清晰度上限；游戏同样资产，但通常镜头更远。
3. **唯一确认缺失的材质输入是反射 cubemap（s6）**：当前用解析天空
   `scSkyRadiance()` 代替 `texCUBE(reflectionSampler)`（文档
   `building-facade-engine-analysis.md` §9.4 已记录为临时近似）。
   解析天空只有三段渐变 + 宽太阳辉光，因此玻璃/幕墙上的“反光”是
   大面积平滑色块，缺少 cubemap 的太阳盘/地平线细节——这正是
   “反光模糊”的来源。
4. MSAA 采样数由浏览器决定（three `antialias:true`），没有显式对齐
   游戏设备档位；它影响边缘而不是贴图清晰度。

## 4. 可选修复（按代价排序）

1. **低成本**：在反射项上叠加一个窄太阳盘
   （`pow(dot(envDir, sunDir), 256..1024)`），让玻璃出现游戏 cubemap
   的高光点；不改贴图链。
2. **中成本**：用 `CubeCamera` 把解析天空渲染成 128² cubemap，
   `reflectionSampler` 改为 `textureCube`；仍无云层细节，但方向/边界
   与 cubemap 采样一致，可加 mip 做粗糙度模糊。
3. **高成本**：从游戏包中定位并导出真实反射 cubemap（需要先找到
   `reflectionSampler` 绑定的资源 TGI），直接上传为 `CubeTexture`。
4. 若目标是“更锐”而不是“更像游戏”，可把 `maxPixelRatio` 上限从 2
   提高，或对建筑 tint 采样加负 LOD bias；这会偏离游戏设置，不建议
   默认开启。

## 5. 需要用户确认的

请给一张**当前 Property Editor 近景截图**（TGI + 白模/精细 + 是否开浮雕），
标出是“墙面贴图糊”还是“玻璃反光糊”：

- 墙面糊：优先核对资产分辨率/是否该建筑本身只有 256²；
- 玻璃/幕墙糊：按 §4.1/4.2 补反射太阳盘或 cubemap。

在没有截图前，不建议直接改采样参数——当前过滤链已与游戏证据一致。
