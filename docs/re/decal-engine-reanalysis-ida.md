# Decal 渲染机制全量重分析（基于 dev 版 IDA 反编译，数据驱动）

> 数据源：`docs/source-code/SimCity-dev-ida.c`（SimCity.unwrapped.exe，2013-01-14 dev beta，IDA Hex-Rays 9.5 + PDB 符号全量反编译，1897901 行，0 个占位函数名）。
> 本文每条机制结论附行号证据；无直接证据的推断单独标注【推断】。
> 本文取代此前基于零售 dump 碎片化反编译 + 运行时猜测的全部 decal 结论。

---

## 1. 资产层：decal info 资源完整字段布局（exe 证据）

`SC::cDecalManager::CreateDecalTexture`（行 987451 起）从 decal info 属性资源读取：

| 属性 ID | 含义 | 证据行 |
|---|---|---|
| `0x0CE5EF4E` | **材质 key**（决定 shader-def 链，经 SP::MaterialManager::GetMaterial 实例化） | 987691-987695 |
| `0x0CE5EF4D` | 单纹理 key（与多纹理互斥） | 987703 |
| `0x0CFEEC90`+i (i=0..3) | 多纹理 key（implicit atlas 模式，连续 ID） | 987713 |
| `0x0CE5EF4F` | texSize (vec2)，存在则走"显式 atlas tile"表 | 987726 |
| `0x0CE5EF60` | atlasSize (vec2)，默认 512×512，存在则走 implicit atlas | 987727 |
| 216395600 (0x0CE5EF50) | decal 实例 ID 数组（key） | 987729 |
| 216395601 / 216395602 | tile position / tile size（vec2 数组，除以 texSize 得 UV） | 987757-987758 |
| 216395603 | aspectRatio 数组（float） | 987759 |
| 216395612-615 | **decalMaterialData0-3（vec4 数组，type 0x80000033）= 字典 colors 四行** | 987761-987768 |

三种表模式：`kDecalTableAtlas`（有 texSize）、`kDecalTableImplicitAtlas`（有 atlasSize，每 decal 最多 4 纹理 216395608-611）、`kDecalTableNoAtlas`（987810-987819）。

cDecal 结构填充：texXform = tilePos/tileSize 归一化（987934-987944、987992-987995）、aspectRatio、numMaterialData（987996-987997）、materialData 四行逐 vec4 拷贝（987998-988056）。

**结论**：shader 链选择 100% 由包内 decal info 资源的材质 key 决定，exe 侧无 shader 名（与此前容器分析一致，现为确证）。

## 2. 顶点流与实例生成：每个 decal 是一个立方体盒

`SC::cDecalData::SpawnDecalInstance`（行 986868-987173）：

**顶点格式字符串**（行 987838-987840）：
```
"V3FI4BT4FT4FT4FT4FT4FT4FT4FT4F" 截断于 3*j+18 (j=numMaterialData)
→ V3F + I4B + T4F×(4+j)
```
- `V3F`：盒角点位置
- `I4B`：4 字节 = `[3×变换索引, materialInfo字节0, materialInfo字节1, materialInfo字节2]`（行 987097、987148-987151）
- `T4F`×4：texXform（贴图 UV 变换）+ m2tex 投影矩阵三行
- `T4F`×j：字典 colors 行（memcpy 自 cDecal::mMaterialData，行 987152）

**盒几何**（行 987099-987158）：8 顶点 = x=±aspectRatio、y=±1、z∈{0, depth}，经 mSubTransform 变换；索引缓冲为 `AllocateIBCube`（行 987084）——**12 三角形的立方体，不是 quad**。

**m2tex 投影矩阵**（行 986946-986988）：由 mSubTransform 求逆构造（scale 取负倒数、平移取反后乘旋转），x 行乘以 `1/aspectRatio`（986971-986976），z 行乘以 `2/depth`、w.z 偏移 `-depth/2` 后同乘（986984-986988）——即把 mesh 空间映射到贴图空间且 z 归一化到 [-1,1] 或 [0,1] 区间。**这套完整 4×3 投影矩阵 + 立方体栅格化是"屏幕空间重建位置 → 投影采样 → 区间外丢弃"的标准体积 decal 结构**。

**materialInfo 三字节**（行 987290-987292，CreateDecal）：float×255 截断 0-255。`ChangeInstanceData`（行 987215-987238）改这 3 字节后 **Clear + 重新 Spawn 整个实例**——引擎接受逐顶点重建的成本，说明该通道设计上是低频（灯开关级别）更新。

## 3. lot/unit 侧：decal set 属性布局与"引擎本就不渲染"的判据

`SC::cDecalSetInfo::FillFromProps`（行 1013252-1013305）：每个 unit 有 4 个 decal set，**setId 互不相同**（构造函数行 1013174/1013183/1013192/1013123）：

| setId | 集合 | 触发条件 |
|---|---|---|
| 0 | Normal（常规） | 默认（Update 行 1013482） |
| 1 | Vandalism（涂鸦） | cGraphicsUnitVandalism，vandalism level bin 驱动 |
| 2 | Vacant（废弃/破洞族） | unit->mControlFlags & 0x400000（行 1013483-101485） |
| 3 | Extractor（采集） | 采集资源地图格 > 0（行 1013487-101493） |

属性 ID = `setId + 0x0D109050 + {0,16,32,48,64,80}`，各 set 元素按索引交错：
- +0：decal ID 数组；+16：transforms；+32：depths（float）；+48：materialInfos（vec3）；+64：renderGroups（uint 哈希名）；+80：machineSpecs（int）

`SC::cDecalSetInfo::CreateDecals`（行 1013308-1013384），**逐 decal 的创建判据**：
1. `depth` 默认 0.1（行 101339）
2. **machineSpecs[i] > graphicsDecalLevel → 直接跳过不创建**（行 101350-101351）——这就是"一个 property 里大量 decal 但引擎只渲染一部分"的 CPU 侧判据：**画质等级门控，引擎逻辑里这些 decal 根本不存在，不是渲染回退**
3. `renderGroups[i] == 0x96AF4B50` → group 强制为 1，否则继承调用方组（行 101346-101349）
4. 材质在当前 RenderType 下无 pass → 整批不画（Render 行 984080-984081）——另一处"合法不渲染"

## 4. 渲染：三个 render group 的相位与调用序（render script 已取证）

`SC::cDecalManager::Render`（行 984026-984145）：`flags` 即 group（`flags < 3`），直接索引 `mBatches[flags]`（行 984078）。每 atlas→batch：`SetShaderData(4, boneMatrices=对象变换)`（984101）→ `SetShaderData(0x206, mObjectData)`（984102）→ `SetRenderState(material, nullptr, numTextures, textures)` → `DrawBufferRanges(VB, IB, IBRanges)`。

**render script 资源已定位并完整解析**（`SimCity_App.package`，type `0x0469A3F7`，group `0x40212005`/`0x40212015`，instance 0-3 对应四个 shader path；二进制大端格式，解析器 `tmp/parse_renderscript.py`，完整转储 `tmp/renderscript/dump_40212015_2.txt`）。`scDecals`（UserDraw，cmdType=5，nameHash `0xE9FFFBFD`）在 shader path 2 脚本中共 10 处调用，分布在 6 个 block；**主相位是 block layer=`0xDA6C50FD`，序列完全确凿**：

```
Push(13)
SetRenderTarget rt[16]=diffuse, rt[71]=lightingFog   ; 写入目标：G-buffer diffuse + 光照雾
SetRaster stage15←rt[14]=depth                        ; 深度 G-buffer 绑为纹理
SetRaster stage14←rt[15]=normal                       ; 法线 G-buffer 绑为纹理
Clear
UserDraw "RenderLocalLights"                          ; 先算局部光照
RenderLayer 3EC5E45B
Push
UserDraw "scDecals" flags=1                           ; group1 先画
UserDraw "scDecals" flags=0                           ; group0
UserDraw "scDecals" flags=2                           ; group2 最后
Pop, Pop
```

**group 语义**（CPU 侧全部赋值点 + render script 调用序）：
- group 0：默认（CreateDecals 继承值、RemoveDrawSet(0) 时，行 952586）
- group 1：decal 的 renderGroup 属性 == 哈希 `0x96AF4B50`（行 101348）。**注意：该魔法值在 SimCityData 全部 14 个包的扫描中 0 次出现**（`crates/dbpf/examples/scan_decals.rs`）——基础游戏没有任何 decal 使用 group1；全息浮空广告可能不走 decal 系统（或为 EP1 特有待复查），group1 调用对空 batch 是 no-op
- group 2：unit 模型处于 DrawSet 0 时（AddDrawSet(0)，行 952559-952560）

其余调用点：block layer=`0xB711AE7B`（主世界 G-buffer pass，绑定 LDRColorPrevFrame/diffuse/lightScalar/normal/depth，flags=1 与 flags=0 分开两处）、block `0x2492C2A6`（415 命令的大 pass，flags=2 后两处 flags=0）、block `0x327902B7`/`0x471FB4A7`（各一处 flags=0，次要用例）。材质按 `mRTMap[renderType]` 决定在哪个 pass 实际参与（行 984080-984081）。

## 5. 投影贴墙 vs 浮空：判定机制（已确证，不再是推断）

**CPU 侧没有任何"法线判定/投影回退/悬浮回退"逻辑**——SpawnDecalInstance 对每个实例无条件生成立方体盒并提交渲染。

**投影贴墙 = 屏幕空间深度/法线重建，render script 级证据确凿**：decal 主相位（§4）把 rt[14]=depth 与 rt[15]=normal 两张 G-buffer 经 SetRaster 绑为纹理，写入目标是 rt[16]=diffuse。配合每顶点携带的完整 4×3 m2tex 投影矩阵（z 行按 2/depth 缩放）与立方体栅格化（IBCube），管线为标准延迟式体积 decal：**PS 采样场景 depth（+normal）重建世界位置 → 乘 m2tex → 三分量在 [0,1] 内才采样着色，否则 discard**。曲面贴合与"不穿透到墙背面"（z 区间裁剪）都由这套机制天然获得。

**浮空 vs 贴墙的分野在材质 shader-def，不在相位**：三个 group 在同一相位连续绘制（1→0→2），相位对三组一视同仁地绑好了 depth/normal；某个 decal 是否做深度重建投影，取决于其材质（decal info prop `0x0CE5EF4E`）编译的 shader-def 是否采样这两张纹理。盒面直绘型 shader（不采样 depth）即视觉浮空。

【对应 OpenSCP】当前 PE 的问题根因全部对上：
- 曲面不贴合/穿透背面：缺"采样深度重建 + m2tex z 区间裁剪"——WebGL 可实现（先渲染建筑深度到纹理，decal pass 重建）
- 全息广告被错误投影：其材质本应是盒面直绘型 shader，PE 对全部 decal 套用了同一投影路径
- decal 画进 diffuse G-buffer、在 RenderLocalLights 之后 → decal 颜色参与后续光照/CLUT/tonemap，夜间自发光调制的完整链路在此

## 6. 动态参数通道：materialInfo 三字节的语义

| 字节 | 语义 | 写入方 | 证据 |
|---|---|---|---|
| x | 静态动画参数（lot props md 三元组第 1 分量） | 仅 CreateDecal 从 lot 属性读入 | 行 101344、987290 |
| **y** | **lightPercent：0=灯全亮，1=灯全灭** | 灯具状态机逐帧驱动 | 行 1011913-101915（visible→y=0，hidden→y=1）、1012413-101416（`y = 1-t`，t 经 LightTransitionCurve 渐变）、1012434-101436（灯灭→y=1） |
| z | 静态动画参数（md 第 3 分量） | 仅 CreateDecal | 同上 |

灯具状态机（kLightNoState/kLightOff/kLightTurningOn/kLightOn/kLightTurningOff，行 1011896-101951）只在 mpLight==nullptr 时走 decal 通道（行 1011906）——** decal 灯与真实光源互斥**：有真灯的建筑用 cLightRenderer::SetVisible/SetKd，没真灯的用 decal lightPercent。

**这直接回答"所有 decal 晚上自发光"问题**：引擎里 neon/招牌 decal 的亮度由 lightPercent(y) 经 `decalNeonBrighten`（`color *= shColorDiff + shColorSpec`）+ `decalLightNeonTube`（`a *= decalMaterialInfo.x`）调制，白天 y→1 应衰减自发光；PE 恒亮 = 没有实现 y 通道的昼夜/状态驱动。

## 7. 假内景：双机制并存（§11  shader 名取证后修正）

**修正（2026-10-05，§11 证据）**：破洞族材质的 compiled-state 着色器名为 **`decalInteriorMap`**（shaders 表 0x700000D2）——破洞 decal 自身就带内景贴图相位，本节"与 decal 系统无关"的强结论作废。并存关系：建筑立面 shader 的 mData 驱动 interior（下文），破洞 decal 有自己的 InteriorMap shader，破洞处露出的内景由后者绘制。

`SC::cGraphicsUnitInteriors`（行 1012688-1012831）计算三个量，经 `GetInfo` 输出 vec3：
- `x = lightsOnPercent × daytimeAmount × nonAbandonedAmount`（行 1012700）
- `y = lightsOnPercent × nonAbandonedAmount`（行 1012701）
- `z = nonAbandonedAmount`（行 1012702）

写入**建筑模型**的 `mModel->mData.xyz`，`mData.w = buildingVariation`（行 954793-954803）。驱动源：
- 入住率/招工率 → lightsOnPercent = amount/capacity×0.7+0.15（行 1012785-101794）
- 废弃标志（controlFlags 0x800/0x8000）→ nonAbandonedAmount 渐隐到 0（行 101796-101800）
- 拆除中（controlFlags 1）→ 全部归 0（行 101809-101813）
- active 标志 0x20000000 可强制 0.85（行 101801-101807、101814-101815）

**假内景在建筑立面 shader 一侧的功能**（配合 material slot0 paletteF32 45×4 参数表 row0 的 interior 参数 + slot1-3 interior 纹理 + slot4 rawBGRA 512×16 调色板）：非破洞区域的立面内景（夜间亮窗）由此驱动。破洞 decal 一侧的 `decalInteriorMap` 负责破洞开口内的内景绘制（待 §11 的片段重组确认其采样链）。OpenSCP 要完整实装假内景，两条链都要做；当前"破洞纯黑"的直接原因是 `decalInteriorMap` 相位未实装。

## 8. 涂鸦（vandalism）

`SC::cGraphicsUnitVandalism`：独立 decal set（setId=1，行 1013123），数量由 vandalism level 全局 bin 驱动；`SetIsVisible(false)` 用 scale=0 的变换隐藏（行 1013023-101029）——**引擎隐藏 decal 的方式是零缩放变换，不是删除实例**。

## 9. 与既有结论的差异（对拍）

| 旧结论（猜测/dump 碎片） | 新证据结论 |
|---|---|
| decal 有法线/回退判定，不合适的会"回退悬浮" | 无任何回退；不渲染只有 3 个合法原因：画质门控、set 切换、材质无 pass（§3/§4） |
| 假内景是破洞 decal 的一部分 | 假内景是建筑 shader 的 mData 驱动功能，与 decal 无关（§7） |
| 投影 vs 浮空由引擎逐实例判定 | 由材质 shader-def + render group 相位决定，CPU 无逐实例几何判定（§5） |
| materialInfo 语义不明/animRatio | y=lightPercent（灯具状态机），x/z=lot 静态动画参数（§6） |
| 招牌动画需要 CPU 逐帧驱动 | exe 内无逐帧 CPU 写动画参数的路径；动画在 shader 内由全局时间驱动，CPU 只在灯开关时重建实例（§2/§6） |

## 10. 待取证清单（下一步，仍按数据驱动）

1. ~~包内 render script~~ **已完成**（§4/§5）：三相位 1→0→2 连续绘制、depth+normal G-buffer 绑定、写 diffuse+lightingFog。剩余细项：layer 哈希名反查（`0xDA6C50FD` 等）、block `0xB711AE7B` 主世界 pass 中 decal 的 RenderType 语义、其余 3 个 shader path 脚本（instance 0/1/3）的差异
2. **renderGroup 哈希名枚举**：0x96AF4B50 在 SimCityData 14 包中 0 命中——需扩扫 EP1/补丁包与 exe 字符串表反查名称；若仍无命中，group1 是预留机制，浮空广告需改从模型/特效系统查证（cGraphicsGameDecals、Swarm effects）
3. **decal 材质 key → shader-def 对照**：从包内 decal info 资源读 0x0CE5EF4E，关联到 shader-def 实例，确定投影型 vs 盒面型 vs SDF 霓虹型各自的 shader-def 实例 ID（这是"哪些 decal 投影、哪些直绘"的最终判据表）
4. **零售版交叉验证**：dev beta（2013-01）与零售（2013-03）若渲染脚本/decal 属性布局有差异，以零售 dump（`docs/source-code/_legacy_ghidra_dump/`）+ 游戏内截图对拍为准；render script 已从零售破解版包提取，可直接对比 dev 版包内同名资源
5. **建筑立面 shader 的 interior 采样链**：slot4 rawBGRA 512×16 调色板 + row0 interior 参数的具体采样方式（假内景实装依据）
6. **PE decal pass 改造蓝图**（基于已确证机制）：① 建筑/地面先渲深度+法线到纹理；② decal pass 逐像素重建世界坐标 ×m2tex，[0,1]³ 裁剪；③ 盒面直绘型 decal（按材质表）跳过重建直接画盒前面；④ 接 lightPercent 昼夜通道（§6）

## 11. 材质链终局：MaterialInfo → CompiledStates → shader 名（2026-10-05 取证）

§10-3 的"材质 key → shader-def 对照"已完成，且超出预期：**不需要猜 shader-def，包内直接存着每个材质的 pass 表与着色器名**。

### 11.1 资源链（TGI 全部确证，exe 行号证据）

decal 字典的材质 key（`0x0CE5EF4E`，instance 如 0x73684EFC）**不是包内资源**，而是 `SP::cMaterialManager::mMaterialsMap` 的运行时注册键（`GetMaterialInternal` 行 453384-453404 只查表不加载，全盘扫描已证实这三个 instance 不存在于任何 .package）。注册发生在 `ReadMaterials`（行 453644-453875），数据源为四组按 shader path（instance 0-3）分副本的系统资源：

| 内容 | type | group | 常量（行 167584-167587） |
|---|---|---|---|
| MaterialInfo（材质 pass 表） | `0x0469A3F7` | `0x40212000` | kGroupMaterialInfo |
| CompiledStates arena（渲染状态对象） | `0x2F4E681B` | `0x40212001` | kGroupMaterialCompiledStates |
| Fragments（着色器片段表） | `0x0469A3F7` | `0x40212002` | kGroupFragments |
| Shaders（着色器记录，含名字） | `0x0469A3F7` | `0x40212004` | kGroupShaders |

均在 `SimCity_App.package`。解析器：`crates/sc-exporter/examples/material_info_dump.rs`。

### 11.2 MaterialInfo 格式（全大端，387 条记录恰好解析到文件尾）

```
version u32 (=1)
loop:
  materialID u32（0xFFFFFFFF 终止）
  numTextures u16；×numTextures: instance u32, group u32, samplerA u16, samplerB u16
  mRTMap 32B = 16 × (numPasses u8, csIndex u8)   ← exe 读进 cMaterialInternal 头 32 字节（行 453758-453764）
  hasCompiledState 16 × u8                        ← 置位则按序消耗一个 arena 导出对象（行 453766-453790）
```

自洽性硬证据：path0/1 全部材质的 hasCompiledState 置位数合计 = **664**，arena section 数 = **664**（path2/3 为 669/671，亦各自相等）——导出序号累计方式确证无误。

### 11.3 三族材质的 pass 表（mRTMap，path0 实测）

| viewer RenderType | 破洞 0x4491DE3A | 涂鸦 0xE5390A98 | 招牌 0x73684EFC |
|---|---|---|---|
| rt0/rt1 | 不画 | 不画 | 不画 |
| rt2 | (0,1) 不画 | (0,1) 不画 | (0,1) 不画 |
| **rt3（decal 相位）** | **1 pass @cs0** | **1 pass @cs0** | **1 pass @cs1** |
| rt4-15（含 rt7 主世界 G-buffer pass） | 1 pass @cs0 | 1 pass @cs0 | **2 pass @cs0+cs1** |

decal 主相位 block `0xDA6C50FD` 在四个 shader path 的 render script 中均由 `RenderType rt=3` 前置调用（path0-3 逐一核对）——**decal 相位 = viewer rt3，全画质档一致**。

### 11.4 compiled-state 段 = D3D9 渲染状态块 + 着色器 ID

arena（0xCAFED00D 头，664 个 0x2000B 材质段）每段：size u32 → 28B 头（**偏移 0x18 = 着色器 ID 0x700000xx**）→ (stateID, value) 对列表。实测三族四个状态：

| 状态 | 着色器 ID | 关键 D3D9 状态对 |
|---|---|---|
| 招牌 cs0 | 0x700000CF | SRCBLEND(0x13)=5 SRCALPHA / DESTBLEND(0x14)=6 INVSRCALPHA |
| 招牌 cs1 | 0x700000D1 | SRCBLEND=**2 ONE** / DESTBLEND=**2 ONE**（加法发光） |
| 涂鸦 cs0 | 0x700000CD | ALPHATESTENABLE(0x0F)=1 + SRCALPHA/INVSRCALPHA |
| 破洞 cs0 | 0x700000D2 | ALPHATESTENABLE + 双状态块（多一组采样器/纹理阶段状态） |

### 11.5 着色器名（Shaders 表 0x40212004，cShaderBase::Read 行 522322-522409：ID → vs/ps version → behaviorFlags → flag&0x10 带名字串）

| ID | 名字 | 归属 | 解读 |
|---|---|---|---|
| 0x700000CD | **`decalProjectSDFLitFront`** | 涂鸦 cs0 | 投影 + SDF + 正面受光 |
| 0x700000CF | **`decalProjectNeonSDF`** | 招牌 cs0 | 投影 + 霓虹 SDF（基础相位：完整图案含背景/图标） |
| 0x700000D1 | **`decalNeonTubeSDF`** | 招牌 cs1 | 霓虹灯管 SDF（additive 发光相位：字体/花纹遮罩亮度动画） |
| 0x700000D2 | **`decalInteriorMap`** | 破洞 cs0 | **内景贴图**——破洞 decal 自带假内景相位 |

behaviorFlags：CD/CF=0x80000018，D1/D2=0x8000001B（低位差 0x03，输入通道差异，待片段重组确认）。

### 11.6 终局结论（取代此前全部猜测）

1. **三族全部走屏幕空间投影**：四个着色器三个名字带 `decalProject`，与 decal 相位绑定 depth/normal G-buffer 互为印证；唯一不带 Project 的 `decalNeonTubeSDF` 是招牌的第二叠加相位。
2. **招牌 = 双相位结构**：`decalProjectNeonSDF`（完整图案：字体+花纹+背景图标，alpha 混合）+ `decalNeonTubeSDF`（灯管发光，ONE/ONE 加法）。霓虹动画属于灯管相位——**动画是字体/花纹遮罩上的亮度渐变，原图案细节始终保留**（与游戏实机观察一致）。PE 当前"丢弃静态图案只留动画色块"的做法在结构上就是错的：两个相位都必须画，动画只调制 additive 相位的强度。
3. **破洞纯黑的根因**：其着色器本体就是 `decalInteriorMap`——破洞不是"黑洞贴图"，而是"内景传送门"贴图；内景采样链未实装则输出黑。修复方向是实装 InteriorMap 的采样（与建筑立面 interior 链共享 paletteF32 row0 interior 参数/rawBGRA 调色板，§7）。
4. **涂鸦清晰度的官方答案**：`decalProjectSDFLitFront` 名字即含 SDF——引擎用 SDF 纹理 + `fwidth` 锐化（与 PE 已实现的 sharp-bilinear 方向一致，但官方实现细节要以其片段重组为准，"LitFront"还暗示按法线/正面受光调制，这解释了涂鸦与墙面颜色的融合）。
5. **rt3 下招牌只画灯管相位（mRTMap=(1,1)）**：decal 相位中招牌的基础图案由 rt7 G-buffer pass 内的 scDecals 调用以 2-pass 绘制（(2,0)）——两组调用覆盖不同 batch group 的招牌实例，组合成完整视觉。PE 改造时两相位都要保留，不能互相替代。

### 11.7 下一步（按序）

1. **片段重组**：解析 cFragmentShader::Read（行 517375+）的 mStateDepPSFragments + Fragments 表（0x40212002，ReadPSFragments 行 519837+），把四个着色器的实际 PS 片段序列与片段源码（decl text）导出——这是"引擎原版 PS 逻辑"的直接移植蓝本，尤其 decalInteriorMap 的内景采样与 decalNeonTubeSDF 的动画参数化。
2. renderGroup batch（0/1/2）与 lot 属性 renderGroups[i]（+64 数组）的实际取值普查——确定三族实例在各组的分布，回答"哪些 decal 在 rt7 画、哪些在 rt3 画"。
3. 0x700000CD 在 Shaders 文件中的另外 3 处引用（6055468/6078584/6102002）——确认是否为其他 shader 的片段依赖或 DirectShader 引用。

## 12. 片段重组终局：四个 decal shader 的原版片段链（Fragments 表全量解析成功）

> 数据源：`SimCity_App.package` Fragments 表（type 0x0469A3F7 group 0x40212002，tmp/fragments_0.bin 1226666B）+
> Shaders 表（0x40212004，tmp/shaders_0.bin）。解析器：`docs/re/decal-shader-fragments/parse_all_fragments.py`。
> 四个 shader 的完整片段序列与片段源码：`docs/re/decal-shader-fragments/shader_frags_*.txt`。

### 12.1 Fragments 表布局终局（EOF 精确匹配验证：VS 1023 条 + PS 1024 条，末偏移 == 文件长）

反编译：`ReadVSFragments`（行 521747-521944）、`ReadPSFragments`（行 519837-520158）、
`cStateDependence::Read`（行 517020-517045）、`cFragmentShader::Read`（行 517375-517495）。

- 表头：version i32(=4) + count i32。VS count=1024 但记录从 fragment 1 起（`i=1; if(v2>1)`，行 521794-521795），实际 1023 条；PS 1024 条（从 index 0 起）。
- VS 记录（15B 前缀）：`i32 A, i32 B, 3×u8, i32 C(flags)` + str1(len i32+体) + str2 + declVec + `[C&2]` 可选串（= 片段名）。
- PS 记录（14B 前缀）：`i32 A, i32 B, 2×u8, i32 C` + str1 + str2 + declVec + `i32 instance, i32 group` + `7×i32` + `numNames i32 + ×string`（version>2）+ `[C&2]` 可选串（= 片段名）。
- **decl 条目 = 名字串 + 4×i16 + i32 + u8（13B）**——反编译只显示 4×i16+i32（行 521907-521911），末尾 u8 为链式全表验证补出的实证字段（少这 1B 全表 1023 条立刻失步）。
- str1 = 代码体，str2 = 辅助函数/uniform 声明（部分片段为空），可选串 = 片段名（如 NullVS/NullPS/decalProject/decalClip…）。
- shader 记录（cShaderBase::Read 行 522322：version=8>7 无头三字段）：`vsVer i32 + psVer i32 + mBehaviorFlags i32 + [flags&0x10: 名字]`，随后 cFragmentShader::Read：循环 `u8 slot`（0xFF 终止）→ VS 列表(u32 count + ×[StateDep 24B + i16 片段idx]) + PS 列表（同构）。StateDep（version>6）= u8 type + 3×i16 + 4×i32(mElements/mIfGroups/mIfNotGroups/mSetGroups) + u8 LOD = 24B。
- **VS 片段索引从 1 起（文件记录 0 = fragment 1）；PS 从 0 起。**

### 12.2 四个 shader 的片段序列（实测，全部为 state slot 2；decalProjectNeonSDF 另有内容相同的 slot 3）

**0x700000D2 decalInteriorMap（破洞）**——VS 5 段 / PS 10 段：
- VS: decalMaterialData1[263] → decalBase[254] → decalProject[256] → decalWorldDirection[260] → decalFloatQuad[257]
- PS: decalMaterialData1[390] → deferredDiffuseEnabled[41] → depthInfo[36] → decalBase[373] → **decalProject[374]** → decalWorldDirection[382] → **decalClip[375]** → setupSHBasic[14] → decalLightInteriorMap[379] → **decalInteriorMap[380]**

**0x700000CD decalProjectSDFLitFront（涂鸦）**——VS 5 段 / PS 14 段：
- VS: decalBase[254] → decalProject[256] → decalWorldDirection[260] → decalMaterialData4SDFSwizzle[267] → decalMaterialInfo[258]
- PS: deferredDiffuseEnabled[41] → overlayBlend4Chan[336] → depthInfo[36] → decalBase[373] → **decalProject[374]** → decalWorldDirection[382] → **decalClip[375]** → **decalClipBack[396]** → decalMaterialData4[393] → **decalSDF[394]** → setupSHBasic[14] → scLightingApplyDeferred[49] → decalMaterialInfo[381] → **decalOpacity[389]**

**0x700000CF decalProjectNeonSDF（招牌基础相位，slot 2 与 3 同构）**——VS 5 段 / PS 8 段：
- VS: decalMaterialData4SDFSwizzle[267] → decalBase[254] → decalMaterialInfoWithObjectData[259] → decalProject[256] → decalNUS[261]
- PS: decalMaterialData4[393] → depthNormalInfo[37] → decalBase[373] → decalMaterialInfo[381] → **decalProject[374]** → **decalClip[375]** → **decalAnimateSDF[384]** → **decalLightSDF[387]**（输出 a=0、rgb=光色，ONE/ONE 加法发光）

**0x700000D1 decalNeonTubeSDF（招牌灯管相位）**——VS 5 段 / PS 12 段：
- VS: **decalBaseCenter[255]**（把顶点压回纹理中心平面：`modelPos += (modelToTexture[2].xyz * invSqrScale) * -texturePosition.z`）→ decalMaterialInfoWithObjectData[259] → decalFloatQuad[257] → decalMaterialData4SDFSwizzle[267] → decalWorldDirection[260]
- PS: deferredDiffuseEnabled[41] → overlayBlend4Chan[336] → decalBase[373] → decalMaterialInfo[381] → **decalFloatQuadNoClip[377]**（不重建深度、不裁剪）→ decalMaterialData4[393] → decalWorldDirection[382] → **decalAnimateSDF[384]** → **decalAnimateSDFDarken[386]** → **decalSDF[394]** → setupSHBasic[14] → decalLightNeonTube[388]

### 12.3 "投影还是浮空"的原版答案：由 shader 片段链内建决定，不存在运行时判定

| 机制 | 片段 | 行为 |
|---|---|---|
| 屏幕空间投影 | decalProject[374] | `farPlaneXYZ` 双 lerp × `depth`（depthMap 解包）重建视线位置 → `camToTexture` 变换得 `texturePosition`；`uvOrig = texturePosition.xy * -0.5 + 0.5` |
| 体积裁剪 | decalClip[375] | `clip(1 - abs(texturePosition))`——投影体积 [-1,1]³ 之外的像素直接丢弃。**这就是"有些 decal 根本不渲染"的引擎逻辑**：像素落在体积外即 kill，不是回退成浮空 |
| 背面裁剪 | decalClipBack[396]（仅涂鸦链有） | `clip(dot(decalWorldDirection, GetDeferredNormal(screenUV)))`——G-buffer 法线与 decal 投影方向点积 ≤0（背面/侧面）即 kill。**投影穿透到模型另一边的根治手段** |
| 浮空（不投影） | decalFloatQuadNoClip[377] | `textureFloatPosition = In.texcoord0.xyz`（VS decalFloatQuad[257] 直接算好），不读 depthMap、不 clip——全息广告/灯管这类浮空 decal 走此路径，永远贴在 decal 自身 quad 上 |

结论：decal 的"投影/浮空"在**资源制作期由 shader 选择固化**。破洞/涂鸦/招牌基础相位 = 投影 + 体积裁剪；灯管相位 = 中心平面压平（decalBaseCenter）+ 无裁剪。OpenSCP 把浮空类 decal 强制套投影路径（悬空广告被投影显示不全）、以及投影类缺少 decalClipBack（穿透到背面）的回归，对照此表即知修复点：投影类必须实装 `clip(1-abs(texturePosition))` 与（涂鸦）背面点积裁剪；浮空类必须走 NoClip 路径不得接深度重建。

### 12.4 decalInteriorMap[380] 完整像素逻辑（破洞 + 假内景原版实现）

前置：decalProject 已采样 `decalTexture`（破洞贴图）放入 Current.color；decalLightInteriorMap[379] 已算光照：
`shColorDiff/shColorSpec/spec/shadow = SimCityLighting(shScreenUV, normalize(decalWorldDirection), worldCameraDirection, …)`，先 `Current.color.rgb *= shColorDiff + shColorSpec + spec`。

decalInteriorMap[380] 本体（逐字）：

```hlsl
const float kSunContributionAmount = decalMaterialData[0].x;   // In.color.x
const float kLightAmount         = decalMaterialData[0].y;     // In.color.y
float3 textureFloatPosition = In.texcoord<t0>.xyz;             // VS decalFloatQuad 输出
float2 interiorUv = lerp(textureFloatPosition.xy, textureFloatPosition.xy * 0.5,
                         textureFloatPosition.z * 0.5 + 0.5);  // 深度抛物线：z 越深 UV 越向中心收缩
interiorUv = interiorUv * -0.5 + 0.5;
interiorUv = interiorUv * texXform.xy + texXform.zw;
float sunMod = saturate(dot(sunSky.mSunDir.xyz, bumpNormal.xyz));
float3 sunColor = sunMod * sunSky.mSunColor.rgb * shadow;
shColorDiff -= sunColor;
shColorDiff *= kLightAmount;
shColorSpec *= kLightAmount;
shColorDiff += sunColor * kSunContributionAmount;
float4 interiorTexture = tex2D(Sampler<s0>, interiorUv);        // 同一张贴图的第二 UV 采样
float3 interiorTextureLit = interiorTexture.rgb *
    (shColorDiff + shColorSpec + spec + interiorTexture.a * kInteriorMapSelfLightMax);  // =16.0
Current.color.rgb = lerp(Current.color.rgb, interiorTextureLit, saturate(decalTexture.a * 2 - 1));
Current.color.a   = saturate(decalTexture.a * 2);
```

关键事实：
1. **破洞贴图与内景贴图是同一张纹理**（Sampler<s0>），两套 UV：投影 UV（破洞外观）+ 内缩 UV（内景）。
2. **decalTexture.a 是破洞↔内景的混合闸**：a<0.5 区域显示破洞纹理（alpha 输出 `a*2`，破洞边缘半透明过渡）；a>0.5 区域显示内景。**破洞"纯黑"的直接根因 = 内景采样未实装或光照项为 0**（kLightAmount=0 会把 shColorDiff/spec 清零，只剩 sunColor×kSunContributionAmount + interiorTexture.a×16 自发光项）。
3. **假内景的景深感来自 UV 抛物线内缩**（z×0.5+0.5 控制 lerp），不需要几何——一个 quad 采样两次即成。
4. 内景自发光：`interiorTexture.a × 16.0`——内景贴图 alpha 通道 = 自发光遮罩（夜间亮窗），与"所有 decal 夜间自发光"的 bug 区分：只有内景相位有自发光，破洞纹理部分完全受光照控制。

### 12.5 招牌双相位原版实现（霓虹动画的全部参数化）

**相位 1 decalProjectNeonSDF → decalLightSDF[387]**（投影 + 体积裁剪，输出加法光）：

```hlsl
float3 decalNUS = In.texcoord<t0>.xyz;                          // VS decalNUS[261]：sqrt(NUSTextureSqr * uniformSqr)，纹理三轴世界尺寸
float materialLightScale = decalMaterialInfo.x * 16.0 + 0.25;
float sdfTextureLength = max(decalNUS.x, decalNUS.y);
float sphereHeight = decalNUS.z;
float hwRatio = sphereHeight * 0.5 / sdfTextureLength;
if (hwRatio < 1) { hwRatio = 1; /* zScale=1/hwRatio 恒1分支 */ }
float circleZ = texturePosition.z;                              // 离纹理中心平面的距离
float4 sdfDists = Current.color;                                // SDF 纹理四通道
float4 circleDists = saturate(1 - sdfDists * 2.0) * hwRatio;    // kMaskCenter=0.5
float4 sphereDistsSqr = circleDists*circleDists + circleZ*circleZ;
float4 animEdge = max(animResults, 0.0);                        // 来自 decalAnimateSDF
sphereDistsSqr += animEdge * animEdge * animRatio * 32;         // 动画相位把光推离字面
float4 lightScales = saturate(1 - sqrt(sphereDistsSqr));
lightScales *= lightScales;                                     // 平方衰减的光晕球
for (int i = 0; i < 3; ++i)
    lightColor[i] = materialLightScale * dot(decalMaterialData[i], lightScales);  // RGB 三通道 = 三组霓虹色
lightColor *= decalMaterialInfo.w;                              // de-power
Current.color.rgb = lightColor; Current.color.a = 0;            // ONE/ONE 加法
```

**相位 2 decalNeonTubeSDF**（中心平面 quad，无裁剪）：decalFloatQuadNoClip 采样 SDF 纹理 → decalAnimateSDF[384]：

```hlsl
float4 animParameters = decalMaterialData[3];                   // 每通道：符号=动画方向(U/V)，整数=chunks，小数=offset
float animSpeed = decalMaterialInfo.y;
float animTime  = frac(gameInfo.x * animSpeed + 0.9999);        // gameInfo.x = 全局时间
float4 animOffsets = frac(animParameters);
float4 animChunks  = max(float4(1,1,1,1), floor(animParameters));
float4 compares  = floor((animTime * 3 - animOffsets) * animChunks) / animChunks;  // 3 相位轮换
float4 uvCompare = lerp(uvOrig.xxxx, uvOrig.yyyy, useV.xyzw);
animResults = uvCompare - compares;                             // <0 = 本通道当前激活
```

decalAnimateSDFDarken[386]：`lightFactor = animResults<0 ? materialTubeLightFactor(=info.z×8+1) : 0.1`；`powerFactor = lerp(0.5, lightFactor, info.w)`；`decalMaterialData[0..2] *= powerFactor`——**动画只调制三组颜色通道的强度**，随后 decalSDF[394] 用 addOverlay（maskCenter=0.5，fwidth 抗锯齿）把 SDF 纹理四通道当遮罩、以调制后的 decalMaterialData[0..2] 上色。**字体/花纹全程是 SDF 遮罩，动画是遮罩内颜色的亮暗流动**——与游戏实机"字体+花纹从暗渐变到亮"一致。最后 decalLightNeonTube[388]：`Current.color.rgb += shColorSpec + spec`（环境高光叠加）。

decalMaterialData 通道布局（VS decalMaterialData4SDFSwizzle[267]）：In.texcoord4-7 的 x/y/z/w 四个分量分别重组为 4 个 float4 = 4 组通道色/参数（[3]=动画参数）。decalMaterialInfo（decalMaterialInfoWithObjectData[259]）：xyz = Current.indices.yzw/255（逐实例数据），w = customParams[(xformIdx/3)/4][(xformIdx/3)%4]（对象自定义参数）。

### 12.6 涂鸦（decalProjectSDFLitFront）原版实现要点

投影 + 体积裁剪 + **decalClipBack 背面裁剪** + decalSDF（addOverlay 四通道遮罩上色，kBlurWidth=1/maskCenter=0.5/fwidth 锐化——官方清晰度答案：SDF 纹理 + 屏幕空间导数抗锯齿，不是多重采样）+ scLightingApplyDeferred（按 G-buffer 法线受光，与墙面光照一致 → 涂鸦与墙面颜色融合的来源）+ decalOpacity[389]：`Current.color.a *= decalMaterialInfo.x`（逐实例不透明度，ALPHATESTENABLE+alpha 混合）。

### 12.7 与 OpenSCP 当前实现的分歧清单（对拍结论 → 修复依据）

| # | 症状（用户观测） | 原版机制（本节实证） | 分歧点 |
|---|---|---|---|
| 1 | 破洞纯黑、无内景 | decalInteriorMap 双 UV 采样 + alpha 混合闸（§12.4） | 内景采样链/光照项未实装；kLightAmount/sunContribution 未从 In.color 读取 |
| 2 | 破洞贴不上曲面 | decalProject 深度重建逐像素求 texturePosition，天然贴合任意曲面（G-buffer 深度驱动） | PE 若用平面拟合/顶点插值投影则必然在曲面失效——必须逐像素深度重建 |
| 3 | 投影穿透到背面（回归） | decalClipBack：dot(decalWorldDirection, worldNormal) ≤0 即 kill | 未实装背面裁剪 |
| 4 | 悬空广告被强制投影显示不全（回归） | 浮空类走 decalFloatQuadNoClip，不重建不裁剪 | 投影/浮空分类依据丢失，全部走了投影路径 |
| 5 | 霓虹变动态色块、丢字体 | 动画只调制 addOverlay 的颜色强度，SDF 遮罩始终完整（§12.5） | 动画实现丢弃了 SDF 遮罩/静态图案 |
| 6 | 静态招牌变纯色块 | 招牌视觉 = 相位1(decalLightSDF 加法光) + 相位2(decalNeonTubeSDF 完整 SDF 图案)，两相位都画 | 只画了一个相位或 SDF 采样链错（package_service.rs 的 decode_decal_entry_rgba vs decode_lot_mask_rgba 口径待查） |
| 7 | 所有 decal 夜间自发光 | 仅内景相位有自发光（interiorTexture.a×16）；招牌发光来自 decalLightSDF/decalLightNeonTube 且受 info.w de-power | 自发光被全局误加 |
| 8 | 涂鸦模糊退化 | 官方 = SDF + fwidth 单次采样锐化（addOverlay kBlurWidth=1） | 多重采样方案偏离原版，应回到 SDF+fwidth |
| 9 | 涂鸦与墙面融合 | scLightingApplyDeferred 按 G-buffer 法线受光 + decalOpacity 逐实例 alpha | 受光/opacity 调制缺失 |
| 10 | 破洞夜间表现 | decalLightInteriorMap 带 shadow 的 SimCityLighting + sunColor 分离（§12.4） | 光照链未按原版分离太阳项 |

### 12.8 遗留核实项

1. 四个 shader 均只在 state slot 2（NeonSDF 另有 slot 3）有定义——slot 0/1（低画质档）无 decal shader 记录，与四画质档 render script 的 rt3 前置调用层级待对齐。
2. PS 片段 extra[7] 字段与 instance/group（本批全 -1）的语义；decal 纹理绑定实际来自材质 mRTMap 的纹理槽而非片段表。
3. `overlayBlend4Chan` 完整源码（导出文件中被截断的长 s2）以 tmp/ps_fragments.json[336] 为准。
