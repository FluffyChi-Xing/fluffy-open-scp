# 归档说明

本文是 `D:\tml\effect-particle-research` 研究目录主报告的仓库归档副本。
数据 JSON / 图集 PNG 不随仓库分发；文中 `../data/` 路径指该外部研究目录。

---

# SimCity (2013) Effect / Swarm 粒子系统文件格式特征分析报告

> 版本：2026-11-01
> 研究对象：`D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData` 下的真实
> `.package`（App / DLC0 / EP1 / Graphics / Game）
> 对照源码：`D:\source-map\rwsrc-v3.7.0.2`（RenderWare Graphics 3.7.0.2）
> 运行时反编译树：`D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree`
> 本报告的数字与字段来自上述真实资源与代码；推测处会明确标注“待证”。

![火效果 4CED418B（Three.js 测试页）](figures/fire.png)

![烟效果 E345D710（Three.js 测试页）](figures/smoke.png)

---

## 0. 结论速览

1. 游戏内 effect 资源**不是 RenderWare 的 `RpPrtStd` 资产**，而是 EA 自研 **Swarm 效果系统**的
   编译态集合（collection），DBPF 资源类型固定为 **`0xEA5118B0`**。RenderWare 3.7 源码在本
   项目中的角色是“粒子 API/数据模型对照基线”，不是该资源的实现体。
2. 一个 effect = **一张 VisualEffect 图**：若干 block（粒子 / MetaParticle / 声音 / 相机 /
   灯光 / 模型 / Distribute / Ribbon / Beam …）指向按类型分段的组件表；block 带局部变换、
   LOD、发射/尺寸/透明度缩放。名字不存明文，导出表只存 **FNV-1 32-bit 哈希**。
3. 粒子参数是**按生命周期归一化时间采样的曲线**（rate / size / aspect / rotation / alpha /
   color），配合发射盒/发射方向区间、速度区间、重力/风力/阻力/径向力、贴图集帧动画和
   draw info（贴图或材质）。
4. **绘制方式由 `cDrawInfo.mDrawMode` 决定**：`15` 表示 `mResourceID` 是材质 ID（走
   `GetEffectMaterial`），`0..14` 是 16 项内置粒子材质表的索引；`mDrawFlags` 的 bit0/1
   在“普通 / light-fog”两套材质表之间选择
   （`SP::cEffectsRenderer::CreateTextureParticleSet`）。
5. 图集帧动画有确定的整数帧公式（见 §9.3），并且**同时输出相邻两帧的 UV + 插值系数**，
   片元着色器在两帧之间插值，而不是硬切帧。
6. 本次研究修正了一个解码错误：**VisualEffect block 的局部变换矩阵/平移是大端 float**，
   旧解码按小端读取会得到 `4.6e-41` 之类的 denormal；修正后 smoke 发射点从原点变为
   `z=500`、fire 为 `z=10`、spark 为 `z=5`（见 §4.5 与 §12）。
7. 真实包统计（只读扫描）：**81 个 effect collection、6,759 个 VisualEffect、6,854 个普通
   粒子块、1,555 个 MetaParticle 块**（三档画质会重复计数，唯一效果数约为 1/3），
   effect 数据合计 **7,099,282 字节**（App 4.85MB、DLC0 1.00MB、EP1 0.94MB）。
   三个画质组（`...00/01/02`）是同一效果的不同 LOD/预算版本。

---

## 1. 研究范围与证据链

### 1.1 真实资源

| 包 | effect collection 数 | 说明 |
|---|---:|---|
| `SimCity_App.package` | 9 | 基础效果：3 个效果 ID × 3 个画质组 |
| `SimCity_DLC0.package` | 66 | 22 个扩展效果 ID × 3 组 |
| `SimCityDataEP1.package` | 6 | EP1 专属：2 个效果 ID × 3 组 |
| `SimCity_Graphics.package` | 0 | 只提供纹理/材质/shader |
| `SimCity_Game.package` | 0 | 不承载 effect collection |

合计 **81 个 collection、7,099,282 字节**。

按包统计（三档重复计数）：

| 包 | collection | VisualEffect | 普通粒子块 | MetaParticle 块 | FastParticle |
|---|---:|---:|---:|---:|---:|
| App | 9 | 5,103 | 4,757 | 1,372 | 6 |
| DLC0 | 66 | 909 | 1,131 | 105 | 0 |
| EP1 | 6 | 747 | 966 | 78 | 0 |
| 合计 | 81 | 6,759 | 6,854 | 1,555 | 6 |

### 1.2 代码与反编译证据

- `crates/sc-properties/src/swarm.rs`：collection 头部、section、VisualEffect、导出表。
- `crates/sc-properties/src/swarm_particle.rs`：粒子 / MetaParticle 字段顺序。
- `crates/sc-properties/src/swarm_material.rs`：辅助材质资源与纹理槽。
- `SP::cEffectsRenderer::CreateTextureParticleSet`：drawMode 15 / 0..14 的材质选择。
- `EA::Swarm::StreamerSetup` / `GenerateQuadInfo`：图集帧、两帧 UV 插值、发射轴向。
- `EA::Swarm::ReadDescription`（`cParticlesDescription` 重载）：粒子字段的权威顺序。
- `cCollection::Read/Write`：collection 顶层布局与导出表。
- `cIRenderer::cDrawInfo` 头文件：draw info 的 24 字节布局。
- `cVisualEffectDescription::cDescriptionRec`：block 记录的字段集合。

### 1.3 与 RenderWare 3.7 的关系

`D:\source-map\rwsrc-v3.7.0.2` 是 RenderWare Graphics 3.7 的 PC 裁剪源码：
`core / world / collis / toon / misc`。粒子插件只有**头文件与闭源静态库**
（`misc/inc/rpprtstd.h`、`rpprtadv.h`、`misc/lib/rpprtstd.lib`），没有 `.c` 实现。
因此本报告只把 RW 3.7 用作：

- 粒子系统**数据模型/API 语义**的对照（`RpPrtStdEmitter`、`RpPrtStdParticleBatch`、
  `RpPTank` 的 blend 模式）；
- 说明“RenderWare 3.7 能解释什么、不能解释什么”。

SimCity 的模型/纹理容器是 **RW4**（`0x2F4E681B`），效果运行时是 **Swarm**
（`EA::Swarm::cEffectsManager` / `SP::cEffectsRenderer`），二者都不在 RW 3.7 源码中。
详见 §10。

---

## 2. 数据链总览

```
Lot 属性 / 规则
  effect_id = Key{ type:0, group:0, instance:FNV1(lower(name)) }
        │  按 32-bit 名字哈希在已加载 collection 中查找
        ▼
DBPF resource 0xEA5118B0  (SimCity_App / DLC0 / EP1)
  group   = 0x40450200|tier  (基础)   / 0x40804300|tier (EP1/DLC 扩展)
  instance= collection id (例如 4CF9B596 / 622B9CD7)
        │  Swarm cCollection::Read
        ▼
Swarm collection (version 4, big-endian)
  ├─ component sections: 粒子 / MetaParticle / 声音 / 相机 / 灯光 / 模型 / Distribute …
  ├─ auxiliary sections: maps(0) / material(1)
  ├─ VisualEffect 数组 (block 图)
  └─ export table: FNV(name) → VisualEffect index
        │
        ▼
VisualEffect → block(type, transform, component index)
        │
        ├─ type 1  → cParticlesDescription (曲线 + 力 + draw info + 图集)
        ├─ type 2  → cMetaParticlesDescription (发射器 + 子组件引用)
        └─ type 3..16 / 32..63 → 其他组件
        │
        ▼
cDrawInfo (drawMode, texture/material id, format, flags, layer, sort)
        │
        ├─ drawMode 15 → 材质资源 (aux type 1) → 参数 slot 2 的纹理
        └─ drawMode 0..14 → kParticleMaterialIDs[16] 内置材质
        │
        ▼
SP::cEffectsRenderer → cTextureParticleSet → GPU quad batch
  （billboard、图集帧插值、加色/透明混合、可选 instancing）
```

---

## 3. 容器层：DBPF / TGI / RefPack

### 3.1 DBPF 头（小端）

`crates/dbpf/src/header.rs` 已实证：

| 偏移 | 类型 | 含义 |
|---:|---|---|
| `0x00` | char[4] | `DBPF`（或 `DBBF` 64 位偏移变体） |
| `0x04` | i32 | major version |
| `0x08` | i32 | minor version |
| `0x24` | u32 | index entry count |
| `0x2C` | u32 | index size |
| `0x3C` | u32 | reserved，必须为 `3` |
| `0x40` | u32 | index offset |

索引项（DBPF，32 字节/项）：

```
u32 typeId; u32 group; u32 unknown; u32 instance;
i32 offset; u32 compressedSize; u32 decompressedSize;
i16 compressionFlag;  // 0=stored, -1=RefPack
u16 flags;
```

> DBPF **头与索引是小端**；进入 effect payload 后，Swarm 流是**大端**。
> 这是本项目最容易踩的字节序陷阱之一。

### 3.2 effect 资源身份

- typeId = **`0xEA5118B0`**（EA Swarm collection）
- group：画质档 + 扩展组
  - `0x40450200 / 01 / 02`：基础包三档画质（同一 instance 的三种预算/LOD 版本）
  - `0x40804300 / 01 / 02`：EP1 / DLC0 扩展组三档
- instance：collection id，例如 `4CF9B596`（主城基础集）、`622B9CD7`（EP1 总集）、
  `BC2C0BE9`、`7A8FF2CB`、`CA341830`、`C83F0064`、`C90AB2FA` 等。

同一 instance 的三档 group 内容高度相似但字节数不同（主集 `284,720` vs `289,609`），
说明 group 编码的是**画质/LOD 预算**而不是不同效果集。

### 3.3 真实 collection 清单（节选）

| 包 | group | instance | 字节 |
|---|---|---|---:|
| App | `40804302` | `622B9CD7` | 1,181,826 |
| App | `40804301` | `622B9CD7` | 1,181,826 |
| App | `40804300` | `622B9CD7` | 1,173,215 |
| App | `40450202` | `4CF9B596` | 289,609 |
| App | `40450200` | `4CF9B596` | 284,720 |
| App | `40450201` | `4CF9B596` | 289,609 |
| App | `40450201` | `BC2C0BE9` | 226,709 |
| App | `40450200` | `BC2C0BE9` | 226,491 |
| App | `40450202` | `BC2C0BE9` | 226,927 |
| DLC0 | `408043xx` | 22 个 instance | 4,039 – 32,605 |
| EP1 | `408043xx` | 2 个 instance | 约 0.94MB 合计 |

完整清单见 `../data/effect_collections.txt`。

---

## 4. Swarm collection 二进制布局

### 4.1 顶层结构

`cCollection::Read`（0x9F6190）与大端实测一致：

```
u16 version = 4
component sections...
  重复:
    u16 type;        // tFXComponentType
    u16 version;     // section version
    u32 size;        // 包含 4 字节 count 的段大小
    u32 count;
    u8  payload[size - 4];
  u16 0xFFFF         // 终止
auxiliary sections...
  重复同上，type = tFXAuxiliaryType；u16 0xFFFF 终止
u16 visualVersion    // 实测 1
u32 visualCount
VisualEffect[visualCount]
export id map:
  重复:
    i32 effectIndex;
    u64 nameId;      // (group << 32) | FNV1(name)
  i32 -1             // 终止
optional name map:
  重复:
    i32 effectIndex;
    char name[];     // NUL 结尾
  i32 -1             // 零售包为空表，紧接 -1
u32 externalEffectCount
u64 externalEffectId[externalEffectCount]
```

> 实现差异：`swarm.rs` 把 name map 之后的 `externalEffectCount` 命名为 `imports`，
> 并吞掉 name map 的 `-1` 哨兵；字段语义应按上面的反编译结果理解。

### 4.2 Section 记录

```
struct Section {
  u16 type;      // 组件/辅助类型
  u16 version;   // 该类型的 schema 版本
  u32 size;      // count 字段 + payload（即 size - 4 = payload 长度）
  u32 count;     // 元素数
  payload;       // count 个连续记录
};
```

读取端先按 `size` 跳到段尾，再处理下一个 section；因此未知类型可以安全跳过。
`type == 0xFFFF` 是终止符，不是有效 section。

### 4.3 主集合 `4CF9B596` 的 section 表（group `40450200`，284,720 字节）

| kind | type | ver | count | payload offset | payload size |
|---|---:|---:|---:|---:|---:|
| component | 1 Particles | 2 | 37 | 14 | 17,705 |
| component | 2 MetaParticles | 2 | 250 | 17,731 | 120,998 |
| component | 4 Sequence | 1 | 3 | 138,741 | 84 |
| component | 5 Sound | 3 | 8 | 138,837 | 400 |
| component | 6 Shake | 2 | 2 | 139,249 | 82 |
| component | 7 Camera | 1 | 6 | 139,343 | 304 |
| component | 8 Model | 1 | 94 | 139,659 | 4,230 |
| component | 9 Screen | 1 | 13 | 143,901 | 1,139 |
| component | 13 Distribute | 5 | 15 | 145,052 | 4,109 |
| component | 15 Animate | 1 | 2 | 149,173 | 224 |
| component | 16 Beam | 2 | 1 | 149,409 | 317 |
| component | 32 external | 3 | 259 | 149,738 | 44,722 |
| component | 33 external | 1 | 1 | 194,472 | 16 |
| component | 37 external | 8 | 10 | 194,500 | 1,344 |
| component | 47 external | 1 | 13 | 195,856 | 1,080 |
| component | 63 external | 1 | 15 | 196,948 | 7,940 |
| auxiliary | 0 Maps | 0 | 5 | 204,902 | 675 |
| auxiliary | 1 Material | 1 | 12 | 205,589 | 899 |

### 4.4 EP1 集合 `622B9CD7` 的 section 表（group `40804302`，1,181,826 字节）

| kind | type | ver | count | payload size |
|---|---:|---:|---:|---:|
| component | 1 Particles | 2 | 1,281 | 620,349 |
| component | 2 MetaParticles | 2 | 154 | 78,934 |
| component | 4 Sequence | 1 | 24 | 1,080 |
| component | 5 Sound | 3 | 241 | 12,050 |
| component | 6 Shake | 2 | 19 | 1,119 |
| component | 8 Model | 1 | 62 | 2,790 |
| component | 9 Screen | 1 | 12 | 1,196 |
| component | 10 Light | 4 | 123 | 19,488 |
| component | 13 Distribute | 5 | 16 | 3,928 |
| component | 14 Ribbon | 2 | 26 | 6,540 |
| component | 15 Animate | 1 | 7 | 896 |
| component | 16 Beam | 2 | 20 | 4,648 |
| component | 37 external | 8 | 136 | 11,838 |
| component | 62 external | 2 | 1 | 461 |
| component | 63 external | 1 | 1 | 740 |
| auxiliary | 0 Maps | 0 | 6 | 810 |
| auxiliary | 1 Material | 1 | 223 | 22,923 |

统计口径：`count` 是**组件数**，一个 VisualEffect block 可以多次引用同一个组件
（例如 3 个粒子 block 共用 component 0 的不同变换），因此 block 数 ≥ 组件数。

### 4.5 VisualEffect 数组与 block

`cVisualEffectDescription` 的写出顺序（`WriteDescription__2`）：

```
u32 mFlags                    // bitset<22>
u32 mComponentAppFlagsMask
u32 mNotifyMessageID
f32 mScreenSizeRange[2]
f32 mCursorActiveDistance
u8  mCursorButton
f32 mLODDistances[count]      // u32 count 前缀
f32 mExtendedLODWeights[3]
u32 mSeed
u32 blockCount
Block[blockCount]
```

Block（`cDescriptionRec`）的序列化顺序（按 `swarm.rs` 解码并经真实包验证）：

```
u8  type                      // tFXComponentType（压缩为 1 字节）
u32 flags
u16 transformFlags
f32 scale
f32 rotation[9]               // 3x3，大端
f32 offset[3]                 // 大端
u8  lodBegin, lodEnd
u32 lodScaleCount
LODScale[count]               // 12 字节/项（emit/size/alpha 三组 begin/end）
f32 emitScaleBegin, emitScaleEnd
f32 sizeScaleBegin, sizeScaleEnd
f32 alphaScaleBegin, alphaScaleEnd
u32 appFlags, appFlagsMask
u16 selectionGroup, selectionChance
f32 timeScale
i32 componentIndex
```

`componentIndex` 的解释空间是**该 block 的 type 对应的组件段**，不是全局数组。
例如 type 1 的 `componentIndex=149` 指 Particles 段的第 149 项。

修正后的真实样本：

| 效果 | block type | component | offset (x,y,z) | rotation |
|---|---:|---:|---|---|
| smoke `E345D710` | 1 | 149 | `0, 0, 500` | 单位矩阵 |
| fire `4CED418B` | 1 | 165 | `0, 0, 10` | 单位矩阵 |
| fire `4CED418B` | 1 | 163 | `0, 0, 5` | 单位矩阵 |

### 4.6 导出表与名字哈希

- 名字 = **FNV-1 32-bit**，输入为小写名字，basis `0x811C9DC5`，prime `0x01000193`：
  `h = h * 0x01000193; h ^= byte`（注意不是 FNV-1a 的 xor-then-multiply）。
- 导出表项 `(i32 effectIndex, u64 nameId)`，`nameId = group<<32 | hash`。
- `effectIndex < 0` 是哨兵；导出项也可以指向 import（`index & 0x7F000000`）。
- 零售 collection 的 name map 为空，实际只有哈希表；所以游戏内 effect 名称不可读，
  UI 只能显示 8 位十六进制。

---

## 5. 组件类型（tFXComponentType）

`include/global/tFXComponentType_ord432.h`（真实反编译头）：

| 值 | 名称 | 画面语义 |
|---:|---|---|
| 0 | `kFXCmptVisual` | 嵌套 VisualEffect（MetaParticle 子效果/组合） |
| 1 | `kFXCmptParticles` | 普通 sprite 粒子 |
| 2 | `kFXCmptMetaParticles` | 元粒子（发射子效果） |
| 3 | `kFXCmptDecal` | 贴花 |
| 4 | `kFXCmptSequence` | 序列 |
| 5 | `kFXCmptSound` | 声音 |
| 6 | `kFXCmptShake` | 屏幕/相机抖动 |
| 7 | `kFXCmptCamera` | 相机 |
| 8 | `kFXCmptModel` | 模型粒子 |
| 9 | `kFXCmptScreen` | 全屏效果 |
| 10 | `kFXCmptLight` | 灯 |
| 11 | `kFXCmptGame` | 游戏逻辑回调 |
| 12 | `kFXCmptFastParticles` | 快速粒子（无普通属性） |
| 13 | `kFXCmptDistribute` | 分布/散布 |
| 14 | `kFXCmptRibbon` | 条带 |
| 15 | `kFXCmptAnimate` | 动画 |
| 16 | `kFXCmptBeam` | 光束 |
| 32..63 | external | 外部/扩展组件（32/33/37/40/41/47/62/63 在真实包中出现） |

真实 block 直方图（按 App / DLC0 / EP1，block 计数）：

| type | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 12 | 13 | 14 | 15 | 16 | 32 | 33 | 37 | 40 | 41 | 47 | 62 | 63 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| App | 1006 | 4757 | 1372 | 12 | 75 | 759 | 75 | 24 | 941 | 114 | 498 | 6 | 372 | 78 | 27 | 129 | 132 | 3 | 501 | 6 | 3 | 126 | 3 | 132 |
| DLC0 | 257 | 1131 | 105 | – | 12 | 183 | 21 | – | 192 | 3 | 150 | – | – | – | – | – | – | – | 18 | – | – | – | – | – |
| EP1 | 128 | 966 | 78 | – | – | 291 | 18 | – | 48 | 18 | 162 | – | 6 | 12 | – | 3 | – | – | – | – | – | – | – | – |

> 普通粒子块合计 6,854；MetaParticle 块 1,555。同一效果的三档画质会重复计数。

---

## 6. 粒子组件（type 1）

### 6.1 字段顺序

以下顺序来自反编译 `ReadDescription` 的 `cParticlesDescription` 重载（0x6511B0），
并与 `swarm_particle.rs` 在 37 + 1,281 个真实粒子上的逐段对齐一致。

| # | 字段 | 类型 | 说明 |
|---:|---|---|---|
| 1 | flags | u32 ×2 | 高字：app/emit 高位；低字：标志位 |
| 2 | life | f32[2] | 生命周期 min/max |
| 3 | preroll | f32 | 预热 |
| 4 | emitDelay | f32[2] | 发射延迟 |
| 5 | retrigger | f32[2] | 重触发 |
| 6 | emitDirection | f32[3] ×2 | 方向区间 min/max |
| 7 | emitSpeed | f32[2] | 初速区间 |
| 8 | emitVolume | f32[3] ×2 | 发射盒 min/max |
| 9 | emitTorusWidth | f32 | 环面宽度 |
| 10 | rateCurve | curve<f32> | 发射率曲线 |
| 11 | rateCurveTime | f32 | 发射率循环周期 |
| 12 | rateCurveCycles | u16 | 循环次数 |
| 13 | rateSpeedScale | f32 | 速率缩放 |
| 14 | speedCurve | curve<f32> | 速度曲线（version 2） |
| 15 | speedVary | f32 | 速度扰动 |
| 16 | sizeCurve | curve<f32> | 尺寸曲线 |
| 17 | sizeVary | f32 | 尺寸扰动 |
| 18 | aspectCurve | curve<f32> | 宽高比曲线 |
| 19 | aspectVary | f32 | 宽高比扰动 |
| 20 | rotationVary | f32 | 旋转扰动 |
| 21 | rotationOffset | f32 | 旋转偏移 |
| 22 | rotationCurve | curve<f32> | 旋转曲线 |
| 23 | alphaCurve | curve<f32> | 透明度曲线 |
| 24 | alphaVary | f32 | 透明度扰动 |
| 25 | colorCurve | curve<rgb> | 颜色曲线 |
| 26 | colorVary | f32[3] | 颜色扰动 |
| 27 | drawInfo | 24B | 见 §6.3 |
| 28 | physicsType | u8 | 物理类型 |
| 29 | overrideSet | u8 | 覆盖集 |
| 30 | tileCountU/V | u8 ×2 | 图集列/行 |
| 31 | alignMode | u8 | 对齐模式（影响 billboard 轴） |
| 32 | frameStart | u8 | 起始帧 |
| 33 | frameCount | u8 | 帧数（0=用满图集） |
| 34 | frameRandom | u8 | 随机帧 |
| 35 | frameSpeed | f32 | 帧速（秒或生命周期归一） |
| 36 | directionalForcesSum | f32[3] | 方向力 |
| 37 | windStrength | f32 | 风力 |
| 38 | gravityStrength | f32 | 重力 |
| 39 | radialForce | f32 | 径向力 |
| 40 | radialForceLocation | f32[3] | 径向力位置 |
| 41 | drag | f32 | 阻力 |
| 42 | velocityStretch | f32 | 速度拉伸 |
| 43 | screwRate | f32 | 螺旋率 |
| 44 | wiggles | cWiggle | 7 条曲线 |
| 45 | screenBloom* | u8 ×4 | 屏幕 bloom 参数 |
| 46 | loopBoxColorCurve | curve<rgb> | loop box 颜色 |
| 47 | loopBoxAlphaCurve | curve<f32> | loop box alpha |
| 48 | surfaces | cSurfaceInfo[] | 表面碰撞 |
| 49 | mapBounce/repulse… | f32 ×7 | 地形反弹/排斥/死亡概率 |
| 50 | altitudeRange | f32[2] | 高度范围 |
| 51 | forceMapID / emitRateMapID / emitColorMapID | u64 ×3 | 贴图/资源引用 |
| 52 | randomWalk | cRandomWalk | 随机游走 |
| 53 | attractorOrigin | f32[3] | 吸引子原点 |
| 54 | attractor | cAttractor | 吸引子强度/范围 |
| 55 | pathPoints | cPathPoint[] | 路径点 |

### 6.2 曲线编码

- `curve<f32>`：`u32 count` + `count × f32`。采样时按 `t ∈ [0,1]` 在 `count` 个键上线性插值。
- `curve<rgb>`：`u32 count` + `count × 3 × f32`（R/G/B）。
- `curve<vec4>`（surface 点等）：`u32 count` + `count × stride`。
- 曲线键**等距**分布在生命周期 0..1；没有显式时间戳。
- 发射率曲线按 `rateCurveTime` 循环（`rateCurveCycles` 控制循环次数），
  不是按粒子生命采样——这是粒子发射与粒子外观两个不同的时间域。

### 6.3 draw info（24 字节）

`cDrawInfo` 头文件：

```
u64 mResourceID;     // group / instance
u8  mFormat;
u8  mDrawMode;
u8  mDrawFlags;
u8  mBuffer;
i16 mLayer;
f32 mSortOffset;
u64 mResourceID2;    // 第二资源（normal/soft 等）
```

- `mDrawMode == 15`：`mResourceID` 是**材质资源**（aux type 1 的 key）。
- `mDrawMode 0..14`：`mResourceID` 是**纹理资源**，同时 `mDrawMode` 又是内置材质表索引。
- `mDrawFlags & 3`：选择 `mParticleMaterials`（普通）或
  `mParticleLightFogMaterials`（受光/雾）材质表。
- `mResourceID == 0xFFFFFFFF_FFFFFFFF` 表示无资源；`0x00000000_7F000000` 是特殊
  “系统默认”哨兵（`sEffectsSystem`）。

### 6.4 图集 / 帧动画

- `tileCountU/V` 是图集列/行数，必须是 2 的幂（引擎用 `mMaskTU/mMaskTV` 位掩码）。
- `frameCount == 0` 表示用满 `tileCountU × tileCountV` 帧；否则只用
  `frameStart..frameStart+frameCount-1`。
- `frameSpeed` 的含义由 flags 高位 bit2 决定（见 §9.3）：
  - bit2 = 0：帧值 = `frameSpeed × age`（秒）
  - bit2 = 1：帧值 = `frameSpeed × age/life`（生命周期归一化）
- 相邻帧的 UV 和 `fraction` 同时输出，shader 做两帧插值。

### 6.5 真实样本

两个集合的粒子样本（节选）：

| 样本 | life | rate | size | alpha | tiles | frameSpeed | drawMode | 纹理 |
|---|---|---:|---|---|---|---:|---:|---|
| `4CF9B596` #0 | 3.0 | 1.0 | 1.0→2.0 | 1→0 | 1×1 | 0 | 2 | `4D272BC5` |
| `622B9CD7` #149 | 15–35 | 27 | 200→800 | 0→0.2→0 | 1×16 | 1.0 (byLife) | 15 | `D7E56466` |
| `622B9CD7` #165 | 2–3 | 100 | 13→30 | 1→0 | 1×32 | 32.0 (byLife) | 15 | `7F29F6A6` |
| `622B9CD7` #163 | 0.75–1.45 | 4000 | 3→1 | 1 | 1×1 | 0 | 15 | — |
| `622B9CD7` #157 | 2.25–2.75 | 0.3 | 4→2 | 0→0.2→0 | 1×8 | 1.0 | 15 | `DF1A43FA` |

`#0` 的粒子头 0x60 字节（相对粒子起点，主集合 payload offset 14）：

```
0000  00 00 00 00 10 00 00 01 40 40 00 00 40 40 00 00  ........@@..@@..
0010  40 40 00 00 BF 80 00 00 BF 80 00 00 BF 80 00 00  @@..............
0020  BF 80 00 00 00 00 00 00 00 00 00 00 3F 80 00 00  ............?...
0030  00 00 00 00 00 00 00 00 3F 80 00 00 00 00 00 00  ........?.......
0040  00 00 00 00 00 00 00 00 3F 88 F5 C3 00 00 00 00  ........?.......
0050  00 00 00 00 3F 88 F5 C3 00 00 00 00 BF 80 00 00  ....?...........
```

- `0x00` flags 高字 = `0x00000000`
- `0x04` flags 低字 = `0x10000001`
- `0x08` life = `3.0`
- `0x0C` life = `3.0`
- `0x10` preroll = `3.0`
- `0x14..0x20` emitDelay/retrigger = `-1.0`（哨兵/禁用）
- `0x24` 起为方向区间；`0x2C` 与 `0x38` 的 `3F 80 00 00` 是 +Z 方向 1.0。

---

## 7. MetaParticle（type 2）

- 复用粒子的“公共发射头”（flags → rate / speed curve），随后追加：
  - 尺寸/颜色/alpha 曲线；
  - `childIndex`（i32）与 `childType`（u32）；
  - death effect 引用；
  - 方向力/全局力/风/重力/径向力/阻力/摆动/表面/地形/随机游走/吸引子/路径。
- `childType == 0` 表示子组件是另一个 **VisualEffect**（不是普通粒子）；
  真实数据里 627 个 App 子引用全部落在有效 VisualEffect 索引。
- `childType` 分布（App）：`0:627, 1:9, 8:3, 14:6, 32:702, 47:9`。
  其中 32 是外部/地形 Brush 组件，不能当屏幕粒子渲染。
- 递归预览需要限制深度/循环；游戏运行时由 `cMetaParticlesEffect::ProcessMetaParticle`
  生成子效果实例。

---

## 8. 材质与纹理（auxiliary type 1）

### 8.1 记录布局

`decode_effect_materials` 与真实 12/223 条材质对齐：

```
u32 group;          // 材质 key group
u32 instance;       // 材质 key instance
u32 shaderGroup;
u32 shaderInstance;
u32 format;         // version 1 才写出；version 0 默认 33
u32 paramCount;
Param[paramCount];
```

Param：

```
u32 group;
u32 id;
u8  kind;
kind 0/1: u32 scalar
kind 2:   u8
kind 3/4: u16 count + count × f32
kind 5:   u16 count + count × u8
kind 6:   u32 textureGroup + u32 textureInstance   // 纹理引用
```

只有 `group == 0 && id <= 7` 的 kind 6 参数被登记为纹理槽；slot 2 是粒子图集。

### 8.2 真实材质 → 纹理 → 图集

| 材质 ID | slot 2 纹理 | 纹理尺寸 | 图集 |
|---|---|---|---|
| `684A2E36` | `C0D189FE` | 128×2048 | 1×16 |
| `93DC33AD` | `A148458F` | 256×256 | 1×1 |
| `FA57D08F` | `D7E56466` | 64×1024 | 1×16 |
| `A9FD260C` | `D7E56466` | 64×1024 | 1×16（smoke #149） |
| `A6E45B67` | `7F29F6A6` | 64×2048 | 1×32（fire #165） |
| `529420F0` | `DF1A43FA` | 32×256 | 1×8 |

`7F29F6A6` 是 32 帧“火焰→烟”序列；`D7E56466` 是 16 帧淡烟；两者都已在本项目
`data/textures/` 下落盘，并在测试页中作为真实图集使用。

### 8.3 drawMode 的运行时语义

`SP::cEffectsRenderer::CreateTextureParticleSet`：

```
if (mDrawMode == 15) {
  material = EffectsResources::GetEffectMaterial(drawInfo->mResourceID);
  ...
} else {
  table = (mDrawFlags & 3) ? mParticleMaterials : mParticleLightFogMaterials;
  material = table[mDrawMode];
  texture1 = TextureManager::GetTexture(mResourceID);
}
```

- `mParticleMaterials[16]` / `mParticleLightFogMaterials[16]` 由 `Init` 从
  `kParticleMaterialIDs` / `kParticleLightFogMaterialIDs` 查出；数组在 exe 静态数据里，
  本次未恢复出每一项对应的 shader 名，因此**不逐个断言 0..14 的混合模式**。
- 可以确定：0..14 是**内置材质索引**，15 是**逐材质覆盖**；
  `drawFlags & 3` 选择受光/雾变体。
- 观察到的 drawMode 分布（EP1 1,281 个粒子）：
  `15:623, 0:312, 2:157, 1:114, 4:70, 6:5`。主集合 37 个：
  `0:11, 2:8, 4:10, 6:1, 15:7`。

### 8.4 图集统计（EP1 1,281 个粒子）

| tiles | 1×1 | 1×2 | 1×4 | 1×8 | 1×16 | 1×32 | 2×2 | 4×1 | 4×2 | 4×4 | 8×1 | 8×4 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 数量 | 843 | 15 | 66 | 16 | 214 | 7 | 24 | 61 | 10 | 17 | 6 | 2 |

主集合 37 个：`1×1:33, 1×4:1, 1×8:1, 1×16:1, 2×2:1`。

---

## 9. 运行时渲染语义

### 9.1 渲染器对象

`SP::cEffectsRenderer`（真实头文件）关键成员：

- `mTextureParticleSets`：活动的纹理粒子集合；
- `mParticleMaterials[16]` / `mParticleLightFogMaterials[16]`；
- `mDrawLists[4]`：4 条绘制列表（按 layer/sort）；
- `mCanUseInstancing` / `mCanUseOffsetInstancing` / `mMaxInstancingIndexCount`；
- `mStaticRenderer`、`mEffectsModel`、`mTextureParticleViewer` 等。

`CreateTextureParticleSet` 为每个 draw info 建立/复用一个 set；set 持有 streamer、
drawInfo、texture1/2、material、materialInfo。渲染时由 `StreamTextureParticles`
把 CPU 粒子流写入 vertex buffer，`RenderTextureParticles` 提交。

### 9.2 四边形与顶点流

`cEffectsRenderer` 目录中存在多套 stream 实现：

- `Stream_Quad_V3F_C4B_T2F.c`（位置/颜色/UV）
- `Stream_Quad_V4F_N4F_C4B_T2F.c`（+法线）
- `Stream_Quad_V4F_N4F_C4B_T4B.c`（+双 UV / 软粒子）
- `Stream_Quad_V4F_N4F_G3F_C4B_T4B.c`（+渐变/额外参数）

说明粒子本质是**面向相机的四边形**；不同 draw mode/material 选择不同的顶点布局，
以支持法线光照、软粒子深度、渐变等。

### 9.3 帧公式（已从反编译逐行核对）

`StreamerSetup`：

```
if (flags.high & 4) { mFSAge = 0;        mFSLife = frameSpeed; }  // 按生命周期
else                { mFSAge = frameSpeed; mFSLife = 0;        }  // 按秒
mDTU = 1/tileU; mDTV = 1/tileV; mMaskTU = tileU-1; mMaskTV = tileV-1;
```

`GenerateQuadInfo`：

```
frameValue = mFSAge * age + mFSLife * (age/life)
frameA = floor(frameValue)
frameB = frameA + 1
frac   = frameValue - frameA
if (frameCount != 0) { frameA %= frameCount; frameB %= frameCount; }
tileX = frameA & (tileU-1)
tileY = (frameA >> log2(tileU)) & (tileV-1)
uv1 = (tileX/tileU, tileY/tileV)
uv2 = 同一帧 B 的 tile 坐标
```

这与测试页 `frameIndices()` 的实现一一对应；`tileU/V` 必须是 2 的幂。

### 9.4 发射轴向与朝向

`StreamerSetup` 调用 `GetParticleAxes(alignMode, sourceTransform, rigidTransform,
emitDir, globalParams, &verticalAxis, &horizontalAxis)`。也就是说：

- 粒子的 billboard 轴不一定是世界 up/right；
- `alignMode` + 效果挂载的 source/rigid transform + 全局参数共同决定局部轴；
- 预览若只做“相机 facing 四边形”会丢失发射方向对齐（例如沿道路/墙面的火焰）。

### 9.5 物理

`cParticlesEffect::UpdateParticlePhysics*`、`ApplySurfaceForces`、`ApplyHeightMapForces`、
`UpdateRandomWalk`、`ApplyAttractor` 等实现了：

- 方向力、风力、重力、径向力、阻力；
- 地形高度图反弹/排斥/杀死；
- 表面碰撞（bounce/slide/death）；
- 随机游走、吸引子、路径点、death effect。

测试页目前只实现方向力/风/重力/阻力 + 生命周期曲线；其余标注为未覆盖。

### 9.6 预算与 LOD

- 3 个 group（`...00/01/02`）是同一效果的画质/LOD 版本，字节数略有差异。
- 运行时按 `mLODDistances` / `mExtendedLODWeights` 选择 block 的 LOD 缩放。
- `cMetaParticlesEffect::CullInactiveSpawnedEffects` 等负责回收子效果实例。

---

## 10. 与 RenderWare 3.7 `RpPrtStd` 的对照

### 10.1 RW 3.7 的粒子数据模型

`misc/inc/rpprtstd.h`（3.7.0.2）：

- `RpPrtStdEmitter`：`flag`、`atomic`、`emtClass`、`prtActive`、`prtBatchMaxPrt`、
  `prtClass`、`activeBatch/inactiveBatch`、`boundingSphere`。
- `RpPrtStdParticleBatch`：粒子按 batch 存储，活跃粒子连续放在 batch 头部。
- `RpPrtStdPropertyTable`：属性 id → offset/size，emitter/particle 是“一块内存 + 属性表”。
- 回调：emitter 11 种（emit/beginUpdate/endUpdate/beginRender/endRender/create/destroy/
  streamRead/streamWrite/streamGetSize/clone），particle 7 种。
- `RpPrtStdEmitterStandard`：seed、maxPrt、force、emtPos/emtSize、emitGap(+bias)、
  prtInitVel(+bias)、prtLife(+bias)、prtInitDir(+bias)、prtSize、prtColor、
  prtDelta2DRotate、prtUV[4]、texture、ltm。
- `RpPrtStdEmitterPrtColor` / `PrtTexCoords` / `PrtSize`：**start/end + bias** 的线性变化。
- `RpPTank`：`srcBlend/dstBlend/vertexAlphaBlend`，`RpPTankAtomicSetBlendModes`
  显式设置源/目标混合。

### 10.2 映射表

| Swarm | RwPrtStd 对应 | 差异 |
|---|---|---|
| VisualEffect block 图 | 无直接对应（RW 是 atomic + emitter 列表） | Swarm 是可嵌套的效果图 |
| Particles 组件 | `RpPrtStdEmitter` + particle class | Swarm 组件按类型分段，可被多个 block 复用 |
| life/emitSpeed/emitVolume | `prtLife/prtInitVel/emtSize` + bias | Swarm 用 min/max + 曲线 |
| rateCurve | `emtEmitGap` + `emtPrtEmit` | Swarm 是时间曲线，RW 是间隔/批量 |
| size/alpha/color 曲线 | `PrtSize/PrtColor` 的 start/end+bias | Swarm 任意键曲线，按生命采样 |
| tile/frame 图集 | `PrtTexCoords` 的 UV 动画 | Swarm 显式 tiles + 两帧插值 |
| drawInfo/material | `RpMaterial` + `RpPTank` blend | Swarm 16 内置材质 + 材质资源 |
| MetaParticle | 无（RW 需自定义 emitter 回调） | Swarm 内建子效果发射 |
| 集合/导出表 | 无（RW 用 clump/atomic 引用） | Swarm 是独立资产 + 名字哈希 |

### 10.3 结论

- **不能用 RW 3.7 的 chunk 解析器读 SimCity effect**：effect 不是 RW 流，没有
  `rwID_...` chunk 头，也不在 clump/atomic 图里。
- RW 3.7 可用来校验**语义**（粒子生命周期、batch、blend、billboard UV 动画），
  但不能用来校验**容器/字段布局**。
- 若后续要做“RW 3.7 风格”的导出/互操作，应把 Swarm 的曲线/图集语义映射到
  `RpPrtStdEmitterStandard` + `RpPTank`，而不是反过来。

---

## 11. 格式指纹（快速识别清单）

看到一个 `.package` 资源，按以下顺序判断是否为 effect collection：

1. DBPF 索引 typeId == `0xEA5118B0`。
2. payload 前 2 字节（大端）== `0x0004`（collection version 4）。
3. 紧接着的 section 记录满足 `type < 0x40`、`version 1..8`、`size >= 4`、
   `count` 合理；段尾是 `0xFFFF`。
4. section 序列里通常出现 type 1（Particles）与/或 type 2（MetaParticles），
   以及 auxiliary type 1（Material）。
5. VisualEffect 段以 `u16 1` + `u32 count` 开始；导出表以 `-1` 结尾。
6. 若把 block 变换按小端读，会得到 `~4.6e-41` 的 denormal —— 这是**大端误读**的
   典型指纹（正确值为 1.0/0.0）。

---

## 12. 本次修正与遗留问题

### 12.1 已修正

- `crates/sc-properties/src/swarm.rs`：VisualEffect block 的 `rotation[9]` 与
  `offset[3]` 由小端改为**大端**读取。修正后真实样本得到单位矩阵与合理发射点
  （smoke `z=500`、fire `z=10`、spark `z=5`），而不再是 denormal。
  该修正已通过 `cargo test -p sc-properties swarm::tests` 与两套真实 collection 的
  完整解析验证。

### 12.2 尚未覆盖

1. **drawMode 0..14 的具体 shader/混合模式**：需要从 exe 静态数据恢复
   `kParticleMaterialIDs` 或从 Graphics 包材质表反查；本报告只确认索引语义。
2. **软粒子深度衰减**：需要 depth texture 与 `Stream_Quad_*_T4B` 的第二套 UV/深度参数。
3. **MetaParticle 的完整发射时序**：子效果实例的位置/继承速度/死亡继承。
4. **surface / random walk / attractor / path**：字段已定位，未做逐参数对拍。
5. **FastParticles（type 12）**：没有普通粒子属性，需单独从
   `cFastParticlesDescription` 恢复。
6. **外部组件 32/33/37/47/62/63**：语义按项目/地形而异，需逐类型调查。
7. **版本 1 粒子的 LE 数组变体**：解码器已做兼容，但零售主路径是 version 2；
   版本 1 的字节序仍需更多样本确认。
8. **Swarm 局部单位**：真实样本（`00B1B104:43E1C000:DBAC1CFC`）的粒子 size 0.5–30、
   block offset 1–6，与建筑 bbox（约 76×49×37 m）同量级，说明效果资产直接以米书写；
   默认按 1.0 渲染。早期厘米假设（0.01）会让普通效果缩小 100 倍，已改为可选的对照档。

---

## 13. 复现方式

### 13.1 探针

```powershell
# 列出包内所有 effect collection
cargo run -p probes --release --example effect_collections -- <package>...

# 统计组件/block/子组件
cargo run -p probes --release --example effect_component_stats -- <package>...

# 导出单个 collection 为 JSON（本报告与测试页的数据源）
cargo run -p probes --release --example effect_swarm_dump -- `
  <package> <group_hex> <instance_hex> <out.json>

# 导出材质纹理
cargo run -p probes --release --example effect_texture_dump -- `
  <package> <group_hex> <instance_hex> <out.png>
```

### 13.2 本目录数据

- `../data/effect_collections.txt`：81 个 collection 的 TGI/大小清单
- `../data/effect_component_stats.txt`：按包的组件/块统计
- `../data/app_main.col.json`：主集合 347 VisualEffect / 37 粒子 / 250 MetaParticle
- `../data/app_ep1.col.json`：EP1 集 1,070 VisualEffect / 1,281 粒子 / 154 MetaParticle
- `../data/textures/*.png`：真实图集（`D7E56466`、`7F29F6A6`、`DF1A43FA`、`BD77BB98`）
- `../data/summary.json`：包级汇总

### 13.3 测试页

见 `../web/README.md`。启动：

```powershell
cd effect-particle-research
python -m http.server 8807
# 打开 http://127.0.0.1:8807/web/
```

支持 `?effect=<hash>`、`?collection=0|1`、`?visual=<index>`、`?warm=<seconds>`。

---

## 14. 术语

| 术语 | 含义 |
|---|---|
| collection | Swarm 效果集合（0xEA5118B0） |
| VisualEffect | 效果图，包含 block 列表 |
| block | 效果图节点，引用某类型的组件 |
| component | 按类型分段的参数记录（粒子/声音/相机…） |
| aux | 辅助资源（maps / material） |
| draw info | 绘制描述：资源、drawMode、格式、层、排序 |
| tile | 图集分块（U/V 列行） |
| FNV-1 | 32 位名字哈希（basis 0x811C9DC5，prime 0x01000193） |
| tier | 三档画质组 `...00/01/02` |
