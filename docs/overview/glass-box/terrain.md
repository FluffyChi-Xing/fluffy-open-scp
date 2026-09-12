# SC::Terrain — 256×256 Typed Map 集

> 深扫笔记（2026-09-12）。扫描范围：`docs/source-code` 下全部 `*Terrain*`、`*Water*`、`*Height*`、`*DataView*`、`SC_SC_UcShaderData*` 共 41 个文件（约 6000 行 IDA 伪 C）。
> 置信度标注：**[高]** = 直接符号名/字符串/常量；**[中]** = 结构+调用关系推断；**[低]** = 类比/外部知识推测。
> 前提：dump 中大量 `.c` 文件只包含析构桩与一段被 IDA 误归属的公共代码（`FUN_00800000`，在 20+ 个文件中逐字节重复）。有效信息集中在少数函数，逐条给出地址。

## 1. Typed Map 体系

### 1.1 类层级（vftable 符号直接证实 [高]）

- `SC::cTerrainMapUint8` —— vftable 于 `SC_cGraphicsGame.c:309`
- `SC::cTerrainMapUint16` —— vftable 于 `SC_cTerrainGround.c:68`
- `SC::cTerrainMapVector4` / `SP::cVector4<cTerrainMap>`（`SC_UcSPVector4_cTerrainMap.c` 暗示 `Uc SP` 命名空间下 Vector4 特化）
- `_anon_0CFBE0D4::cTerrainMapUint8FX` —— 匿名命名空间的 **FX（渲染侧）包装**（`SC_cGraphicsGame.c:350-351`）
- `SC::cTerrainMapGPU` —— 独立文件，析构 `FUN_00bee580` 释放 +8 处带 LOCK 的引用计数对象（疑为设备纹理句柄）[中]
- `SC::E::cTerrainMap`、`SC::G::cTerrainMap` —— 见 §4 前缀讨论

**共享布局 [高]**：Uint8/Uint16/Vector4/E_/G_ 六个类的析构是**同一函数** `FUN_00bffdb0`，释放 `this+0x14` 成员后走 `EA::RefCountTemplate<int>` 虚表 → 同一模板 `cTerrainMap<T>` 的不同实例化，**数据指针在 +0x14**。Vector4/GPU 析构（`FUN_00bee580`）额外先对 `this+8` 的引用计数对象加锁递减 → Vector4/GPU 的缓冲在 **+8** 且可被外部共享 [中]。

### 1.2 数量：13 张 map

`SC_cTerrainGame.c` 三个常量 getter [高]：

| 函数 | 返回值 | 解读 [中] |
|---|---|---|
| `FUN_00bdd1f0` | `0x0e556570` | 地形游戏资源/属性的 32 位 hash ID |
| `FUN_00bdd200` | `0xd` = **13** | **map 数量** |
| `FUN_00bdd210` | `0x10000` = **65536 = 256×256** | **格子总数** |

13 张 typed map 与已知系统（高度、水表、地面水、污染、土地价值、矿、油、风力、土壤肥沃度、湿度/生态、草/森林密度、岩层、游荡代理权重…）数量级吻合；具体每张的名字在 dump 中未以字符串出现，无法逐一命名 [低]。

层/effect 分派（§6 DrawLayer）中 14 个相邻 hash 常量与"13~14 种层"互证 [中]。

### 1.3 各 typed map 语义线索

| Map | 直接证据 | 语义推断 |
|---|---|---|
| `cTerrainMapUint16` | cTerrainGround 初始化 `FUN_00bf10e0` 创建，随后 `FUN_00be6040(0x100,0)` [高] | 地面/地层索引图（Uint16 支持 >255 种地层 id）[中] |
| `cTerrainMapUint8`（服务器） | cGraphicsGame 启动时构造并挂 `this+0x98` [高] | 单通道属性图（资源量/水位/代理权重标量）[中] |
| `cTerrainMapUint8FX` | 包装同一对象（§4） | 同上的 GPU 只读镜像 [高] |
| `cTerrainMapVector4` | 存在 + `UcSPVector4` 特化 | 多通道图：疑为水表（水位+流量+浊度）或数据视图 RGBA [低] |
| `cShaderDataTerrainForestPS/VS` | RTTI [高] | 专门的"森林"着色器 → 存在森林/树密度图 [高] |
| `cShaderDataMapContourInfo` | `SC_cMaterialGroundDataViewBlendColors.c:135` [高] | 数据视图等值线参数 → 污染/地价以等值线呈现 [高] |
| EcoMap（Combiner/List） | `FUN_00bf70b0` 两组 vec3 常量 [高] | Combiner 持两组 vec3 参数（权重/渐变端点），多张 eco map 加权合成视图 [中] |

## 2. 256×256 的证据

1. [高] `FUN_00bdd210` 返回 `0x10000` —— 唯一直接常量。
2. [高] `FUN_00bdd200` 返回 13（互证 map 数）。
3. [高] `SC_cTerrainGround.c:72` `FUN_00be6040(0x100, 0)`、`SC_cTerrainHeightMap.c:139/161` 两次 `FUN_00be8090(map, 0x100, fmt)` —— 0x100=256 作尺寸实参。
4. [中] cTerrainGame 对象巨大：被引用字段 `+0x23ce4`（草层数据）、`+0x24f18`、`+0x27990/+0x27995/+0x27a46/+0x27a4f` → 对象 ≥ 0x27a50 ≈ 162.6 KB → **内联多张 map 或含 Uint16 图**。
5. [中] 逐格步长：`SC_cTerrainSystem.c::vf15` 中 `*(short*)(esi+0x1cc) * 0x20 + *(int*)(edi+0xa8)` —— 32 字节步长一维索引（物理/碰撞格），一维化方形地图。
6. [中] 世界尺度：`FUN_00bf1070` 的 `(x * (1.0f/DAT_0103d43c) + 1.0f) * DAT_00da307c`。若城市 2048m/256 格，则 `DAT_0103d43c`=格边长 8.0m、`DAT_00da307c`=高度单位 [低]。
7. [低] 反证：terrain 文件中无 `>>8`/`&0xff` 的 x/z 解包 —— 逐格遍历在 SIMD/任务代码中（未入 dump）。

## 3. CPU→GPU 同步（`SC_cGraphicsGame.c::FUN_006883c0`）[高]

```
serverGame = param_1+0xc;
src6       = serverGame->vtable+0xac();   // 6 dword 描述符（疑:尺寸/范围/min-max）
map        = new SC::cTerrainMapUint8(初始化自 vtable+0xb4);
this+0x98  = map;                          // 客户端持有的服务器 map 引用
fx         = new(0x28) cTerrainMapUint8FX;
fx[3..8]   = src6;                         // 拷贝地图描述符
fx[9]      = map;  map->refcount++（+4 原子自增）
register(0xce226af, fx);                   // 注册到渲染服务
this+0xa0  = service->vtable+0x6c(0x5f384bd7);
```

**结论 [中]**：同步不是逐帧深拷贝，而是 **FX 包装对象共享持有 CPU 端 map（引用计数）+ 复制静态描述符**；GPU 上传在 FX 绘制路径中（未捕获）。`cTerrainMapGPU` 是 D3D 资源侧对应物 [低]。

## 4. E / G 前缀含义

`SC::E::cTerrainMap` 与 `SC::G::cTerrainMap` 是同一模板的两次实例化（析构同 `FUN_00bffdb0` [高]）。

**[中] E = Eco（服务器/模拟侧）、G = Graphics（渲染侧）**：
1. GlassBox 侧官方术语即 "Eco"（`GB_cEcoGameDescription/Effect/HandlerBase`、`GB_cEcoSwarmMap`）；
2. `SC_cTerrainSystem.c::FUN_004053e0` 引用全局名 **`u_TerrainForServer`** [高]，存在显式的"服务器地形"注册路径（hash `0xc714f929`/`0x54dd43da` 安装/卸载对）；
3. 渲染侧具名 FX 类与 G 副本角色重叠。

## 5. 水系统

1. **"water" 层按名挂接 [高]**：`SC_cTerrainWater/Stratum/Rock.c` 共享 `thunk_FUN_00bee960`：构造字符串 `"ground"`/`"water"` 经 `FUN_0094c380` 按名查询，结果写 `+0x40`（hasGroundEffect）`+0x41`（hasWaterEffect）——材质缺失时层自动停用。
2. **水层数据面 [中]**：`+0x38`=层ID、`+0x3c`=数据源+0x48（每层 map 切片基址）→ 每层在 `+0x48` 起有自己的 map 切片。
3. **渲染 uniform [高]**（RTTI）：`cShaderDataWaterHeightInfo / NormalsInfo / ChoppyInfo / StrataPS` —— 水由 Height/Normals/Choppy（碎波）/Strata（分层色带）四组数据驱动。
4. **Tessendorf 波浪 [高]**：`SC_cTessendorfWater.c` 独立类（`FUN_00bfd830` 比较 hash `0x44edd9c` 置 `+0x110`）→ 水面为 Tessendorf 谱法（FFT），Height/Choppy/Normals 三纹理是其输出。
5. **水纹理集 [中]**：`cWaterTextureSet::FUN_00c05e70` 绑定 VS 槽 `0x101/0x102/0x103`、PS 槽 `0/1/2`（格式 1,2,2）、槽 `3`（格式 3,2,2）。
6. **2k_terrain_WATER 关系 [低]**：dump 无 `2k_terrain` 字符串；最可能经 `cWaterTextureSet` 0~3 槽进入，需资源包验证。

## 6. 地形编辑（Terraform 写入路径）

- **`cTerrainLayer::DrawLayer` [高]**（`FUN_00bfc270`，带 profiler 字符串）：按 32 位 hash 分派 **14+ 种 effect/消息类型**：`0x0f1f7671, 0x0825751a, 0x0eb085c7, 0x18382c78/79, 0x26ca3d03, 0x2cc9f32b/c, 0x3a73d0c5, 0x3d30ab76, 0x46735c96/97, 0x789b1630, 0x912bfe8f, 0xa8709e67, 0xda249aa0, 0xf0cb12d5`。成对 hash（…78/79、…96/97、…2b/2c）疑为 do/undo 消息对 [低]。
- **effect map set [中]**：每层 `+0x3c` 指向 `数据源+0x48` 的 effect map 切片 → 编辑器写 effect map，DrawLayer 把 map 变绘制调用。
- **GrassLayer2 [中]**（`FUN_00c07530`）：响应 `0xa6a3a238 / 0x83b18b81` 消息，直接读写 `cTerrainGame+0x23ce4` 的内联 map 缓冲 → 服务器 cTerrainGame 是编辑/模拟写入的最终落点。
- **高度图/表面初始化 [高]**：`FUN_00c01f80`（`SC_cTerrainSurface.c::FUN_00c00ee0` 亦调）装配两张 256 项 map（+0x54/+0x60）、`cMIDataT<cShaderDataTerrainNormalsPS>`（资源 id 0x28e）、三个 hash 查询 `0x02bb709b / 0x6392b73e / 0x21918f0b`（各配 0xd0 字节渲染状态）→ 高度修改后经三条属性管线通知法线/区域/info 重算 [中]。

## 7. 渲染接口（shader 数据语义）

| 类（RTTI [高]） | 语义推断 |
|---|---|
| `cShaderDataTerrainRegionVS` | VS 常量：城市区域→纹理区域映射（世界→地图 UV、格尺寸），所有地形材质共享 [中] |
| `cShaderDataTerrainNormalsPS` | 法线/坡度数据块，高度图变更时重建 [中] |
| `TerrainHeightPS / TerrainInfoPS / TerrainForestPS / ForestVS` | 高度、通用 info、森林密度（VS 顶点密度+PS 叶色）[低] |
| `cShaderDataMapContourInfo` | 数据视图等值线参数（间隔/颜色/宽度）[中] |
| `cShaderDataWater*` 四件套 | Tessendorf 波场 + 分层水色 [高] |
| `SP::cMIDataT<T>` | `cMaterialInfo::cData` 模板：**shader 常量缓冲宿主**（+6 word=资源 id，+8 起=数据）[高] |
| `cGroundTextureSet / cWaterTextureSet / cTerrainTextureSet / cSkirtTextureSet` | 四套纹理槽集合（地面/水/地形/裙边）[高] |
| `cGraphicsDataView`（`FUN_00809f00`） | 数据视图层表：每层描述符 **0x5c 字节**，颜色在 `+0x3c~+0x44`（RGBA），按层 id 匹配打包下发 [中] —— 污染/地价上屏通道 |
| `cTerrainDataView` | 服务器→图形按位置查询接口（取标量/颜色，缺省 1.0）[中] |

另：`SC_cTerrainGfx.c::FUN_00bec090`（查 `+0x27995` 标志后走精细/粗糙双路径采样）疑为 `GetHeight(x,z)` 的 LOD 双路径 [中]。

## 8. 对 OpenSCP 的可复用结论

| # | 结论 | 证据 | 置信度 |
|---|---|---|---|
| 1 | 城市格 **256×256=65536**，一维 index 寻址；OpenSCP 用 `[u8;65536]`/`[u16;65536]`/`[[f32;4];65536]` 平铺数组 | FUN_00bdd210；`*0x20` 寻址 | 高/中 |
| 2 | map 模板统一 `cTerrainMap<T>`：数据@+0x14；缓冲可共享（Vector4/GPU @+8）。OpenSCP 用 `Arc<RwLock<Vec<T>>>` | FUN_00bffdb0 / FUN_00bee580 | 高/中 |
| 3 | 约 **13 张 typed map** + 14 种层/effect 类型；先建 13 槽注册表再校准名字 | FUN_00bdd200 + DrawLayer hash 计数 | 中 |
| 4 | 服务器（Eco）与图形（G/FX）各一份：图形侧**共享持有**服务器 map（引用计数）而非深拷贝 | FUN_006883c0 | 高 |
| 5 | E=Eco(服务器)、G=Graphics | 文件名 + u_TerrainForServer | 中 |
| 6 | 层按字符串名 "ground"/"water" 查询 effect；每层持共享大缓冲的切片视图 | FUN_00bee960 | 高/中 |
| 7 | 地面层用 Uint16（地层 id >255）；地面查找表 256 项；高度/表面两张 256 项 LUT + 法线块 | FUN_00bf10e0 / FUN_00c01f80 | 高 |
| 8 | 水 = Tessendorf（三纹理由类内生成）+ StrataPS + 4 槽纹理集；水表 map 是 "water" 层切片；2k_terrain_WATER 资源未在代码出现 | §5 | 高(结构)/低(资源) |
| 9 | 数据视图上屏：层描述符 0x5cB、颜色@+0x3c、等值线 7×float4 → 污染/地价可视化直接复用 | FUN_00809f00 / 0x292 | 中 |
| 10 | terraform 写入落点 = cTerrainGame 内联 map；高度改动后 3 条属性管线重算法线/区域/info → OpenSCP terraform 后触发同等三路脏标记 | FUN_00c07530 / FUN_00c01f80 | 中 |
| 11 | 物理高度场同步未捕获（文件只有桩），OpenSCP 需自行设计 | — | 高(缺失) |
| 12 | 跨模块常量全是 **32 位属性 hash**（0xe556570 等）；FNV/CRC32 均不匹配（已程序化验证）——Maxis 私有 hash，存档兼容前必须破解 | 全文 + 比对实验 | 高 |

### 遗留缺口

- 13 张 map 的逐一名称与序号（属性 hash 未破解）。
- `FUN_00bea050`（资源数据灌入各 map）与 `FUN_00bebf70`（map set 工厂）本体未入 dump。
- `2k_terrain_WATER` 引用、cTerrainMapGPU 上传函数、terraform 工具写图代码 —— 需补扫 Tool/Renderer/资源管线段。
