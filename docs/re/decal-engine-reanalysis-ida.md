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

## 4. 渲染：三个 render group 分相位绘制

`SC::cDecalManager::Render`（行 984026-984145）：`flags` 即 group（`flags < 3`），直接索引 `mBatches[flags]`（行 984078）。每 atlas→batch：`SetShaderData(4, boneMatrices=对象变换)`（984101）→ `SetShaderData(0x206, mObjectData)`（984102）→ `SetRenderState(material, nullptr, numTextures, textures)` → `DrawBufferRanges(VB, IB, IBRanges)`。

**group 语义**（CPU 侧全部赋值点）：
- group 0：默认（CreateDecals 继承值、RemoveDrawSet(0) 时，行 952586）
- group 1：decal 的 renderGroup 属性 == 哈希 `0x96AF4B50`（行 101348）——名称未知，可扫包内现有 renderGroup 值枚举；**全息/浮空广告的最有力候选**
- group 2：unit 模型处于 DrawSet 0 时（AddDrawSet(0)，行 952559-952560）——即模型本体进入主批次时的 decal 组

注册入口：`RegisterCommand("scDecals", RenderEntry, mDecalManager)`（行 759207-759212）。**三个组由包内 render script 资源在帧内不同相位各调用一次**——相位顺序（以及该相位绑定的全局纹理，如场景深度）在包内 render script 资源里，exe 不可见。【待取证：从包内提取 render script，确认三个 scDecals 调用点的相位与绑定】

## 5. 投影贴墙 vs 浮空：判定机制（回答核心问题）

**CPU 侧没有任何"法线判定/投影回退/悬浮回退"逻辑**——SpawnDecalInstance 对每个实例无条件生成立方体盒并提交渲染。差异完全来自两条数据通道：

1. **材质（shader-def）**：decal info 的材质 key 决定片元着色器行为。m2tex 三行 + 深度缩放恒在顶点里，投不投影由 shader 用不用这套矩阵决定。
2. **render group → 渲染相位**：group 1（0x96AF4B50）在不同相位绘制。该相位若在世界几何之后、且 shader 直接画盒面 → 视觉浮空（全息广告）；相位若带场景深度 → 投影贴墙。

【推断，高置信】投影贴墙（含曲面贴合）的实现是标准体积 decal：decal pass 绑定场景深度纹理（render script 相位级绑定，exe 不可见），PS 重建世界位置 → 乘 m2tex → 三分量都在 [0,1] 内才采样，否则 discard。证据链：完整 4×3 投影矩阵（含 z 深度行，986984-986988）、立方体而非 quad（IBCube）、decal 在独立相位绘制（render script 命令）。**这同时解释穿透问题：z 区间裁剪正是"不穿透到墙背面"的机制；不做 z 裁剪或不用深度重建，必然穿透/漂浮。**

【对应 OpenSCP】当前 PE 直接在盒面/墙面上采样贴图的方案：
- 曲面上无法贴合（缺深度重建）
- 穿透到模型背面（缺 m2tex z 区间裁剪）
- 全息广告被错误投影（group 1 应走"直接画盒面"分支而非投影分支）

## 6. 动态参数通道：materialInfo 三字节的语义

| 字节 | 语义 | 写入方 | 证据 |
|---|---|---|---|
| x | 静态动画参数（lot props md 三元组第 1 分量） | 仅 CreateDecal 从 lot 属性读入 | 行 101344、987290 |
| **y** | **lightPercent：0=灯全亮，1=灯全灭** | 灯具状态机逐帧驱动 | 行 1011913-101915（visible→y=0，hidden→y=1）、1012413-101416（`y = 1-t`，t 经 LightTransitionCurve 渐变）、1012434-101436（灯灭→y=1） |
| z | 静态动画参数（md 第 3 分量） | 仅 CreateDecal | 同上 |

灯具状态机（kLightNoState/kLightOff/kLightTurningOn/kLightOn/kLightTurningOff，行 1011896-101951）只在 mpLight==nullptr 时走 decal 通道（行 1011906）——** decal 灯与真实光源互斥**：有真灯的建筑用 cLightRenderer::SetVisible/SetKd，没真灯的用 decal lightPercent。

**这直接回答"所有 decal 晚上自发光"问题**：引擎里 neon/招牌 decal 的亮度由 lightPercent(y) 经 `decalNeonBrighten`（`color *= shColorDiff + shColorSpec`）+ `decalLightNeonTube`（`a *= decalMaterialInfo.x`）调制，白天 y→1 应衰减自发光；PE 恒亮 = 没有实现 y 通道的昼夜/状态驱动。

## 7. 假内景：不属于 decal 系统（重大方向修正）

`SC::cGraphicsUnitInteriors`（行 1012688-1012831）计算三个量，经 `GetInfo` 输出 vec3：
- `x = lightsOnPercent × daytimeAmount × nonAbandonedAmount`（行 1012700）
- `y = lightsOnPercent × nonAbandonedAmount`（行 1012701）
- `z = nonAbandonedAmount`（行 1012702）

写入**建筑模型**的 `mModel->mData.xyz`，`mData.w = buildingVariation`（行 954793-954803）。驱动源：
- 入住率/招工率 → lightsOnPercent = amount/capacity×0.7+0.15（行 1012785-101794）
- 废弃标志（controlFlags 0x800/0x8000）→ nonAbandonedAmount 渐隐到 0（行 101796-101800）
- 拆除中（controlFlags 1）→ 全部归 0（行 101809-101813）
- active 标志 0x20000000 可强制 0.85（行 101801-101807、101814-101815）

**假内景是建筑立面 shader 的功能**（配合 material slot0 paletteF32 45×4 参数表 row0 的 interior 参数 + slot1-3 interior 纹理 + slot4 rawBGRA 512×16 调色板），与 decal 系统无关。**破洞内露出内景 = 立面 shader 在破洞遮罩区域采样 interior 层**，不是破洞 decal 带内景纹理。OpenSCP 要做假内景，必须在建筑 shader 里实现 mData 四分量 + interior 采样链，而不是在 decal shader 里想办法。

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

1. **包内 render script 资源**：三个 scDecals 调用的相位顺序、各相位绑定的全局纹理（验证场景深度 → 投影型 decal 的深度重建）→ 包扫描工具提取
2. **renderGroup 哈希名枚举**：扫全部 lot props 的 renderGroups 数组值，统计 0x96AF4B50 出现位置（哪些 decal 是 group1），并尝试反查名称
3. **decal 材质 key → shader-def 对照**：从包内 decal info 资源读 0x0CE5EF4E，关联到 shader-def 实例，确定投影型 vs 盒面型 vs SDF 霓虹型各自的 shader-def 实例 ID
4. **零售版交叉验证**：dev beta（2013-01）与零售（2013-03）若渲染脚本/decal 属性布局有差异，以零售 dump（`docs/source-code/_legacy_ghidra_dump/`）+ 游戏内截图对拍为准
5. **建筑立面 shader 的 interior 采样链**：slot4 rawBGRA 512×16 调色板 + row0 interior 参数的具体采样方式（假内景实装依据）
