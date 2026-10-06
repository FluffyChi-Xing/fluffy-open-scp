# Dev Beta (RL) 对 OpenSCP 未决问题的线索价值（2026-10-04）

> 三篇分析之三。样本判定见 `dev-beta-sample-inventory.md`，
> 三方对比见 `dev-beta-vs-retail-diff.md`。
> 本文按**当前未决问题**逐条给出 RL 带符号反编译能提供什么、不能提供什么。
> 提取产物均在 `tmp/dev-sample-analysis/out/`（`fn_*.txt` 为函数体，
> `dev_function_names.txt` 为 17,346 个限定名清单）。

## 〇、总判断

RL 样本的最大价值不是"早期实现更简单"，而是
**`SimCity.pdb` + 1.9M 行带符号反编译**构成的 RL 分支 ground truth：
此前 PL dump 分析中的 `FUN_`/`DAT_` 猜测链，现在可以逐项用
"同分支同名函数"验证；而 RL↔PL 的**差异本身**（属性语义、管线代际）
也是结论适用边界的标尺。对以下未决项，3 项获实锤推进、
2 项获方向性证据、2 项确认仍是死路。

## 一、decal 体积盒 ×5 因子（遗留验证项 §五.1）——**获实锤**

PL 侧结论（`decal-engine-alignment.md` 证据 C）：盒基 =
旋转 × scale(+0x34) × DAT(0.5) × DAT(10.0)，两个魔数靠 exe 常量直读。

RL 侧 `SC::cVolumeDecalManager::UnitCreateModel`
（`fn_SC_cVolumeDecalManager_UnitCreateModel.txt`）把同一公式
以**字段名**写出：

```c
v20 = v13->mWorldTransform.mScale * 0.5;
v16->mX.x = (v13->mWorldTransform.mRotation.xAxis.x * v20) * 10.0; // mY/mZ 同构
v16->mX.w = mTranslation.x + x_local * mScale;  // mFlags&2 时先把平移旋入本地系
```

- **×0.5×10 = ×5 在两分支都是真实引擎行为**，不是 PL 反编译误读——
  遗留疑问"×5 vs 我们游戏对拍校准的 ×2"收窄为：
  差异必在 **unit scale 字段语义** 或 **texXform 窗口只取盒的一部分**，
  与公式本身无关。frida 对拍清单可缩到这两个点。
- 附带实锤：`mTuning.xyz` = 属性 **0x0DA76A05/06/07**（229075461/2/3，
  typeId 13 float），缺省回退管理器默认值——证实
  decalMaterialData 前 3 行的属性来源，与 PL 侧"0xf..0x12 槽位"对上了名。

## 二、0xCAAD8C9 / 0xCAAD8CA 属性语义——**分支间发生了演化，结论不可跨分支混用**

- **RL**：`UnitCreateModel` 读 **0x0CAAD8C9**（212523209）为
  **int（typeId 9）volType**：`volType > 0` 才异步加载体积数据
  （`GetVolumeDataForModelAsync`）并创建 decal 槽；`volType == 0` 则销毁。
  即 RL 里 0xCAAD8C9 是"体积类型"开关。
- **PL**（既有结论）：0xCAAD8C9 = materialInstance（材质上传），
  0xCAAD8CA = float 可见类别（`category > 0` 判据）。

**警示**：同一属性 ID 在两分支语义不同。我们数据面（package 属性）
来自 PL 时代内容，PL 语义适用；但若引用 RL 反编译理解属性含义，
必须意识到这一漂移。这也解释了 RL 反编译里看不到
0xCAAD8CA 的原因（该属性后加）。

## 三、实例材质数据的量化——**获实锤**

`SC::cDecalManager::ChangeInstanceData`
（`fn_SC_cDecalManager_ChangeInstanceData.txt`）：

```c
v4->mByteMaterialData[0..2] = clamp(inMaterialData.x/y/z * 255.0, 0, 255);
// 然后 ClearDecalInstance + SpawnDecalInstance 重生实例
```

实例材质数据 = **3 通道 uint8（float×255 量化）**，改值即重生 decal。
这是"引擎量化合成"链路在实例级的官方实现证据，
与 OpenSCP 现行"阈值128×Color线性×2"口径的**输入端**对齐；
注意此处是 cDecalManager（旧逐 decal 路径），体积路径的量化在
shader/实例数据侧，两者不要混。

## 四、shader-data 注册表——**RL 整表在手，SDF 动画 uniform 的 C++ 端可定位**

`SC::RegisterShaderData`（`fn_SC_RegisterShaderData.txt`）给出
**ID → 尺寸 → 上传函数**全表（节选）：

| ID | 尺寸 | 上传函数 |
|---|---|---|
| 0x2A3 | 0x300 | `UploadBatchColors` |
| 0x2A4 | 0x300 | `UploadBatchDatas` |
| 0x2AA | 0x50 | `UploadDeferredLight` |
| 0x2AB | 0x140 | `UploadGlobalLights` |
| 0x2BF | 0x904 | `UploadBatchInstanceInfo` |
| 0x2C4 | 0xC04 | `UploadImpostorParticles` |
| 0x2C9 | 0x14 | `UploadSimPalette` |
| 0x2CD/0x2CE | 0x204/0x44 | `UploadImpostorAttachments/Angles` |
| 0x2A8/0x2A9 | — | `UploadViewNearCorners/ViewFarCorners` |

另有 `GB::RegisterShaderData`、`SC::RegisterTerrainShaderData`（7KB，
地形系整表）同批提取。PL 把这些重构成了 27 个 `UcShaderData*` 类
（见 diff 篇 §3.1）——**RL 表的 ID 与 PL 的 Uc 类名可建立映射**，
"SDF 动画 uniform 由哪个上传函数、哪个槽位喂入"这一未决项
从"运行时全抓"缩小到"按 ID 对表查 upload 函数体"。
相关符号：`SP::PushShaderData/PopShaderData/SetShaderData/ShaderDataApplied`、
`SP::GetShaderDataSize`。

## 五、破洞簇 / 内景体积数据链——**获方向性证据（Danielsson SDF 实锤）**

破洞（decalInteriorMap 族）的"内景体积"在 RL 侧有完整带名实现：

- `SC::GetVolumeDataForModelAsync`
  （`fn_SC_GetVolumeDataForModelAsync.txt`）：先按
  **资源 type 0x0D9F9914**（228563220，AO volume info）异步查资源；
  查不到则走运行时构建任务链。
- `cBuildVolumeData::CreateDistanceField`
  （`fn_cBuildVolumeData_CreateDistanceField.txt`）：
  `CreateMaskFromMesh`（mesh → W×H×D 位掩码，W 按 32 对齐）→
  `InitDeltaFromBitMask` → **`SC::Danielsson`**（经典 Danielsson
  欧氏距离变换，8 连通 SED）→ `FillDensityFromDeltas` →
  每槽 **W×H×D 个 uint8 密度体**。同族还有 `CreateAO`、
  `ComputeBBox`、`UpdateTexture`。
- 配套：`SC::cGraphicsUnitInteriors::*`（FillFromProps/GetInfo/OnBeat/Update）
  是内景（房间链）的图形侧挂载点。

对"破洞簇（1813da18 RW4 DXT 载体）渲染收口"的意义：
**内景体积 = mesh 构建的 3D SDF 密度场 + AO，资源键 type 0x0D9F9914**——
可以先在 package 里按此 type 查是否有烘焙体积资源（有则免构建），
渲染时按密度场裁剪内景采样，而不是只做 BackSide 盒。
RW4 DXT 纹理本身仍走既有 rw4/raster.rs 路径，本链解决的是
"体积数据从哪来、什么格式"。

## 六、夜态 bloom——**确认 exe 内无线索，方向仍为数据/脚本侧**

在 17,346 个 RL 符号中 `bloom` **零命中**，PostProcess/ToneMap/
BrightPass 亦无（仅音频 PostProcess 与无关命中）。结合 PL 侧
"bloom = 引擎后期，不逐像素模拟"的既定决策：两分支 exe 都没有
bloom 的命名实现 → bloom 几乎确定由**渲染脚本/材质数据**组合
（RenderScript 资源）实现，继续静态扒 exe 是死路；
建议改查 `SP::cMaterialManager::ReadRenderers`/`LoadRenderers`
加载的 renderer 资源与游戏 RenderScript 包。

## 七、材质/shader-def 路由——**静态死路维持，但 RL 侧给了阅读入口**

`SP::cMaterialManager::ReadShaders/ReadMaterials/ReadFragments`
已带名提取（12.8KB/8.7KB）。可见结构：fragment shader 表 +
显式 fragment hashtable + 材质引用解析。既有死路结论
（materialInstance→族映射 = eco 编译期产物，静态不可读，见
deadend dead-mtt129cw）**不受本样本影响**——RL 同样是数据驱动。
但若要理解 eco 产物的**消费侧**格式（fragment 如何拼成 shader-def、
render state 在哪里应用），RL 带名代码是目前最好的阅读材料。

## 八、地形/渲染管线代际提醒

RL 无 `cTerrainWater/Rock/TextureSet`、无 `UcShaderData*`（均 PL 新增）；
RL 地形 shader 数据走 `SC::cTerrainGfx::UpdateShaderData`、
`cShaderDataTerrainForestPS` 等旧式类。即**两分支地形/建筑渲染
存在 building4→building5 式代际差**：OpenSCP 对拍素材若混用两分支
截图，须先确认所对机制在哪一代管线。

## 九、样本解决不了什么（诚实清单）

1. materialInstance → shader 族映射：仍是数据侧，RL 同样不可静态读，
   frida 捕获计划不变（RL 构建甚至更难启动：需 Origin 激活，
   这也是附 DRM 密钥的原因）。
2. 混合状态（恒 0 alpha 纹理的可见性）：render state 仍在
   shader-def 数据里，PL/RL 都需运行时抓 `SetRenderState`。
3. MSAA：两版 exe 均无 MultiSample 字符串，维持"不可定谳"。
4. PL 新增系统（离线存档、CoT、地形新管线）在 RL 里**不存在**，
   相关问题（如本地存档格式）只能继续用 PL dump。

## 十、建议的后续动作（按性价比排序）

1. **pdbparse/DIA 提取 RL 全局符号↔地址表**（含 DAT_ 真名）：
   一次脚本换全部 RL 常量命名，后续所有 RL 阅读提速。
2. 按 §四注册表把 SDF 动画相关 `Upload*` 函数体读完，
   定位 uniform 槽位（替代一次 frida 会话）。
3. 在 package 资源中普查 type **0x0D9F9914**（AO volume info）——
   有烘焙数据则破洞内景可走"读体积"而非"运行时构建"。
4. 若需要 PL 的函数名：以 RL 带名反编译为参照，
   对 PL dump 做"字符串引用 + 反编译形态"锚定重命名
   （从 `cVolumeDecalManager` 等共有类开始）。
