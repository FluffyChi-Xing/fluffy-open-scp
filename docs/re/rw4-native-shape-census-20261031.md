# RW4 模型格式取证与 native-single 编译器（2026-10-31）

日期：2026-10-31。背景：干净包 + 菜单包入游戏后**加载存档崩溃**。用户指认
编译 RW4 形状异常（105 个碎片 mesh / 14MB / 大量重复 section）。本文记录
全量取证结论与新编译器。

## 一、原版形状普查（crates/rw4/examples/rw4_census.rs）

扫描 SimCity_Graphics + SimCity_Game + SimCityDataEP1 全部 **11348 个 RW4
模型**：

| 维度 | 分布 |
|---|---|
| mesh/文件 | {0: 3333, 1: 6132, 2: 1877, 3: 2, 4: 4}——**上限 4** |
| 材质/文件 | 与 mesh 严格 1:1 |
| 大小 | p50 57KiB、p99 991KiB、**max 5.3MB** |
| fixup 对 | 几乎全 0（偶见 1-2） |
| BBox/Collision | 有 mesh 的文件几乎都是 1+1 |
| 内嵌 Raster(0x20003) | 10960/11348 文件有（≥1，2×2 占位） |

结论：v3 编译器「逐 GLB mesh 一 RW4 mesh」（商业 105 mesh / 729 section /
14MB）是**孤儿形状**，用户观察成立。

## 二、引擎加载语义（docs/source-code/SimCity-dev-ida.c）

- 入口 `cGraphicsResourceFactory::ReadResourceArena_1`（ida:459366, 0053B440）：
  读 152B 头（rw::core::arena::Arena）→ 要求 `baseResourceDescriptors[0].m_size
  >= 0x98` → **按 4 个描述符 size 顺序读完全文件，`mRecordSize == 0` 才算
  成功**；任何不匹配 → false → 模型资源加载失败。
- **4 个描述符 = 头 word16-23（字节 0x40..0x60），8B 对 (offset, m_size)**：
  d0=(0, sie)（header+payload+index 区）、d1=(16, total-sie)（blob 区）、
  d2=(4,0)、d3=(1,0)。我们与原版**一致**（writer 早已正确）。
- `Arena::FixupStart`（ida:363460）：dictStart（w12=sib）、sections（w13=0x98）、
  numUsed（w8=section 数）按偏移重定位；type ∈ [0x10010,0x1001F] 的 section
  按 base resource 块重定位，**0x10030 等 blob 按绝对 pos**（与我们的写法一致）。
- 消费端 `AddGeometryFromArena`（ida:500717）：draw 列表由 0x2001A（MMA）驱动，
  **多 mesh 合法**（EP1 有 1839 个双 mesh 文件）；BBox 缺失回退 ±1 盒；
  Collision 缺失由运行时 KD-tree 缓存补建。
- 加载存档路径 `SP::CreateModelInstance → ConstructFromArenaResource`。

## 三、头部逐字段对拍（tmp/rw4-compare/）

我们与原版（native_2mesh / nighthouse）**逐字段一致**，唯一例外：
头字节 0x64（word 25）——原版 mesh 类文件 = `0xC0_XXXX`（低 16 位为 8 的
倍数，随文件变化），无 mesh 类 = `0x1B8`；我们写死 **0x40**。语义未破译，
`ReadResourceArena_1`/`FixupStart` 均不读取（疑似流系统元数据）。
native_model 的 vertex_format 12B/元素契约经核实**无回归**（取证报告 B1 为误报）。

## 四、native-single 合并编译器（scene.modelShape = "merged-single"）

`native_model::compile_lotm_merged` / `compile_merged_primitives`：

1. 全部源 GLB 几何贪心合并为最少 mesh（每 mesh ≤65535 顶点，u16 索引帽；
   超 65535 顶点的单 primitive 显式报错）。
2. 只保留材质 0（六槽）——其余材质 raster 资源仍入库但模型不引用
   （完整贴图正确性需图集烘焙，排后续）。
3. 追加 BBox（32B = 2×vec4，placement 后几何）与 2×2 占位 Raster
   （NightHouse 逐字节副本）；**只追加不重排**（mesh/VA/TA/MMA payload 内
   部按 section 编号互引，前插会整体移位失配）。
4. 头 word25 = 0xC003F8（NightHouse 形状克隆）。
5. 商业样本实测：729 section/105 mesh/无 BBox → **16 section/2 mesh/
   1 材质/BBox/Raster**，头描述符与原版同构。

schema：`scene.modelShape: "merged-single"`（缺省仍为逐 mesh 分区契约）。

## 五、崩溃隔离验证进展（2026-10-31）

用户以 `000_test_building_v3.package`（= 商业建筑_单mesh测试，merged-single
2 mesh 模型 same-TGI 覆盖）进游戏**加载存档不崩溃**——合并模型通过游戏
加载，隔离测试初步通过。待确认：楼体在游戏内可见（贴图仅材质 0，属预期）。
菜单测试当时误用了 v3（无菜单资源）；修复版菜单包已以
`000_menu_park_test.package` 部署至离线目录（md5 301df1d8…）。

## 五·零、原始指引（待用户实测）

SimCityData 同批入包的有：两个 `_clean` 包（same-TGI 覆盖 + 105 mesh 模型）、
菜单包（新身份）。**逐包隔离**：

1. 只留 `商业建筑_单mesh测试.package`（same-TGI 覆盖 + 2 mesh 原版形状
   模型）：加载含该商业楼的存档。
2. 不崩 → 多 mesh/形状问题实锤，merged-single 转正，图集烘焙排期。
3. 仍崩 → 换 property-only 覆盖（LOD 指回原模型）再测，隔离 property
   scene 写回 vs 模型；菜单包最后单独测（新身份 + 模板继承）。

## 六、遗留

- w25 语义（0xC0XXXX/0x1B8）未破译；原版普查 CSV 在 tmp/census/。
- merged-single 的贴图正确性：需把 98 套六槽纹理图集烘焙为 1-4 套
  （decode → blit → UV 重映射），排 M2。
- COLLISION_MESH 未生成（引擎自建，拾取/物理精度后续再评估）。
