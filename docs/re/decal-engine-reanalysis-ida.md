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

1. ~~包内 render script~~ **已完成**（§4/§5）：三相位 1→0→2 连续绘制、depth+normal G-buffer 绑定、写 diffuse+lightingFog。剩余细项：layer 哈希名反查（`0xDA6C50FD` 等）、block `0xB711AE7B` 主世界 pass 中 decal 的 RenderType 语义、其余 3 个 shader path 脚本（instance 0/1/3）的差异
2. **renderGroup 哈希名枚举**：0x96AF4B50 在 SimCityData 14 包中 0 命中——需扩扫 EP1/补丁包与 exe 字符串表反查名称；若仍无命中，group1 是预留机制，浮空广告需改从模型/特效系统查证（cGraphicsGameDecals、Swarm effects）
3. **decal 材质 key → shader-def 对照**：从包内 decal info 资源读 0x0CE5EF4E，关联到 shader-def 实例，确定投影型 vs 盒面型 vs SDF 霓虹型各自的 shader-def 实例 ID（这是"哪些 decal 投影、哪些直绘"的最终判据表）
4. **零售版交叉验证**：dev beta（2013-01）与零售（2013-03）若渲染脚本/decal 属性布局有差异，以零售 dump（`docs/source-code/_legacy_ghidra_dump/`）+ 游戏内截图对拍为准；render script 已从零售破解版包提取，可直接对比 dev 版包内同名资源
5. **建筑立面 shader 的 interior 采样链**：slot4 rawBGRA 512×16 调色板 + row0 interior 参数的具体采样方式（假内景实装依据）
6. **PE decal pass 改造蓝图**（基于已确证机制）：① 建筑/地面先渲深度+法线到纹理；② decal pass 逐像素重建世界坐标 ×m2tex，[0,1]³ 裁剪；③ 盒面直绘型 decal（按材质表）跳过重建直接画盒前面；④ 接 lightPercent 昼夜通道（§6）
