> 深扫笔记（2026-09-12）。范围：SC_cZoningGame、SC_cToolMetaLotZone、SC_cMetaLot/LotGroup/PlacementLotGroup、SC_cLotBasedZoneHandler、SC_cGraphicsZone/Bucket、SC_cBuildPlacementLotWalker、各重叠过滤器、SC_cZoningSplineGeoHelperFuncObj、DestructionSplineGeo*、SC_cRoadManager（约 30 文件）。
> 置信度：【高】= 多处独立证据互证；【中】= 单一强证据+合理推断；【低】= 纯推测。偏移为 32 位 this 相对偏移。

SC::Zoning/Lot 分区与地块成长系统 深扫笔记

> 扫描范围：`D:\rust\packages\fluffy-open-scp\docs\source-code`（IDA 伪 C 转储，348 个文件）。
> 所有偏移均为 x86 32 位 this 指针相对偏移；`FUN_xxx` 为未命名函数。置信度标注：【高】= 多处独立证据互相印证；【中】= 单一强证据+合理推断；【低】= 纯推测。

---

## 0. 文件清单（`ls | grep -iE "lot|zoning|zone|parcel|build|growth|spline"` 全量）

| 文件 | 大小 | 内容（实读结论） |
|---|---|---|
| SC_cZoningGame.c | 30.4KB | cZoningGame：地图载入注册 zone/lot、pending 队列、消息处理 |
| SC_cToolMetaLotZone.c | 28.4KB | 分区画笔工具（含共享的 zone 密度查询助手 FUN_008a6xxx） |
| SC_cMetaLot.c | 11.0KB | cMetaLot：跨多个 lot 的"超级地块"聚合 + 拖动重分区 FUN_00854020 |
| SC_cLotGroup.c | 6.4KB | cLotGroup：角点/lot 分组 + FUN_00852550 从 lot 初始化 |
| SC_cPlacementLotGroup.c | 10.0KB | cPlacementLotGroup：放置用连续 lot 链聚合（min/max 包围） |
| SC_cLotBasedZoneHandler.c | 9.2KB | **开发循环核心 FUN_007e46d0**（按 lot 派生建筑，含时间预算） |
| SC_cGraphicsZone.c | 16.2KB | zone 着色渲染：0x1B4 元素数组，字符串 `s_GraphicsZone_draw_lots` |
| SC_cGraphicsLotBucket.c / SC_cGraphicsZoneBucket.c | 213B | 仅析构壳 |
| SC_cBuildPlacementLotWalker.c | 1.3KB | 放置 walker：尺寸适配判断 FUN_007df640、接受写入 FUN_007e2b60 |
| SC_cToyPlacementLotWalker.c | 1.1KB | 同上简化版 FUN_007e1e70/FUN_007df680 |
| SC_cCollectZonePathLotsMap.c | 1.4KB | cIRibbonWalker ctor |
| SC_cConflictingZonedOverlappedLots.c | 3.2KB | 冲突检测：同路不同 zone 的重叠 lot（FUN_00865000） |
| SC_cFirstZonedOverlapper.c | 1.2KB | 重叠 lot 枚举器（FUN_008645a0，用 cConflictingZonedOverlapper） |
| SC_cConflictingZonedOverlapper.c | 1.2KB | 过滤器：同路且同 zone 才算一伙（FUN_008640b0） |
| SC_cUnzonedWithZonedOverlapper.c | 1.6KB | "未分区但有分区邻居"过滤器（FUN_00864480） |
| SC_cLotsShareRoadAndZone.c | 0.9KB | 共享过滤器基类逻辑（FUN_00863f20：同 +0x10 道路 id） |
| SC_cIZoningParcelOperator.c | 0.5KB | 纯虚基类 `SC::cIZoningParcelOperator::vftable` |
| SC_cIFilterLotsBase.c / cIFilterParcelIndexesBase / cILotOverlappersFilterBase / cIOperateOnLotsBase / cOperateOnPathParcelsBase | ~0.5KB | 纯虚接口壳（vtable 符号名） |
| SC_cZoningSplineGeoHelperFuncObj.c | 3.3KB | **parcel 创建 FUN_0085aaf0**（0x4C 数组写入）+ 0xC 候选三元组 |
| SC_cDestructionSplineGeoFuncObj.c / SC_cWholeDestructionSplineGeoFuncObj.c | ~2.4KB | 拆除时沿 spline 重建几何（0x40 步长 id 数组） |
| SC_cRoadManager.c | 959 行 | 道路属性表载入、0x50 步长每路记录（间接相关） |
| SC_cToolAddTowerLevel.c | — | 密度升级工具（复用 FUN_008a6xxx 密度阈值查询） |

关键全局辅助（未在本目录转储，仅见调用）：`FUN_006d9960`=64 位 id→对象指针解析；`FUN_006d99b0`=索引→对象；`FUN_006d7600/006d7640`=存在性测试；`FUN_006373d0`=全局游戏数据指针；`FUN_006de6f0/FUN_006de8f0`=读 64 位资源 id / 判非零。

---

## 1. 数据结构与字段布局

### 1.1 Lot 对象（经 FUN_006d9960 解析的"lot/corner 池对象"）【中，多文件拼合】

```
+0x00/+0x04  u64  lot ID（cLotGroup FUN_00852550 写入自身 +0x3c/+0x40）
+0x0c        u8   活动标志（FUN_00854020 要求 !=0 才可重分区）          【中】
+0x10        i32  所属道路/path id，-1=无（FUN_00865000、FUN_00864010、FUN_007df440 均以 +0x10 判道路）【高】
+0x14..0x28  6xf32 前排位置/尺寸三元组×2（cPlacementLotGroup +0x12c..0x140 逐项拷贝；
                  FUN_008548b0 对 +0x14/0x18/0x1c 取 MIN、+0x20/0x24/0x28 取 MAX）【中】
+0x38/+0x3c  f32  角点/高度标量（FUN_00854020 与地面高度 local_34 比较差值平方）【低】
+0x50/+0x54  u64  派生 id（cMetaLot +0x6c/+0x70 来源）                  【低】
+0x68        f32  权重/面积（cMetaLot FUN_00852740 加权混合中心点用）     【中】
+0x6c..0x74  3xf32 位置 A（MIN 聚合）；+0x78..0x80 3xf32 位置 B（MAX 聚合）【高】（FUN_008546e0/008548b0）
+0x84/+0x88  u64  端点 corner id A                                     【高】
+0x8c/+0x90  u64  端点 corner id B                                     【高】
+0x94        u64  分区(zone)资源 id（FUN_006d9ba0 写入；FUN_00864480/008640b0 读取比较）【高】
+0xa0/+0xa4  f32  宽度对（临路面宽/背面宽，cMetaLot 取 max/min）          【中】
+0xa8        f32  （Airplane/GUI 经 0x5c 数组间接引用的资源 +0xa4/+0xa8）【低】
+0xcc/+0xd0  vec  覆盖的 parcel(cell) 索引数组（元素 4 字节索引 → 0x4C 数组）【高】
+0xc8        i32  （另一对象上）建筑资源 id，-1 空 —— 见 1.3           【中】
+0xec..0xf4  3xf32 中心点（cMetaLot 按 +0x68 权重混合累计）            【高】
+0x38(记录)  vec  候选建筑表，元素 0xC 步长（见 §4）                    【中】
```

证据文件：`SC_cLotGroup.c`、`SC_cPlacementLotGroup.c`、`SC_cMetaLot.c`、`SC_cBuildPlacementLotWalker.c`、`SC_cLotsShareRoadAndZone.c`、`SC_cConflictingZonedOverlappedLots.c`。

### 1.2 cMetaLot（聚合器，SC_cMetaLot.c FUN_00851910/00852740/00853f90）【高】

```
+0x01        f32  权重和（混合中心分母）
+0x08/+0x0c  u64  端点 corner id A（随成员 lot 伸展更新）
+0x10/+0x14  u64  端点 corner id B
+0x18/+0x1c  f32  宽度 max / min（来自 lot+0xa0/+0xa4）
+0x30..0x3a  3xi32 加权中心（按 lot+0x68 混合 lot+0xec..0xf4）
+0x58/+0x5c  vec  成员 lot id 列表（8 字节/项）
+0x64(+100)  u32  zone 类别掩码（用于测 parcel+0x1c）
+0x68        u64  分区资源 id（lot+0x50/+0x54；FUN_00853f90 从 lot 拷贝）
+0x68(+0x19) u32  掩码非 0 时：清除所有成员 lot 覆盖 parcel 的分区位（FUN_008524f0）
+0x74        i32  索引（-1 有效）
+0x9c(+0x13) u64  哨兵 -1,-1 = "未初始化端点"
+0xa0..      bool 标志（+0x80/+0x81）
```
cMetaLot 本质是"沿同一街区连续 lot 的聚合视图"，用于把一笔分区涂抹作用到整段。

### 1.3 Parcel / Cell 数组 —— 0x4C 步长【高，本任务最重要的布局】

分配点：`SC_cZoningSplineGeoHelperFuncObj.c` FUN_0085aaf0：
```c
iVar4 = FUN_006228c0();              // 分配新 parcel 索引
iVar8 = iVar4 * 0x4c;                // ← 0x4C 步长
iVar1 = *(int *)(*(int *)(param_1 + 8) + 0x20);   // 数组基址 = zoning系统对象+0x20
*(iVar1 + 0x20 + iVar8) = -1;  *(iVar1 + 0x24 + iVar8) = -1;   // 建筑/占用 id = 空
*(iVar1 + 0x10 + iVar8) = iVar7; *(iVar1 + 0x14 + iVar8) = uVar6; // 分区资源 id
FUN_0085cd70(...);                   // 写入 spline 几何（角点）
*(... + 0x1c + iVar8) = 4;           // zone 类别位掩码（初始位 2）
*(... + 0x1c + iVar8) = FUN_0085d800();  // 再写为掩码
*(... + 0x28 + iVar8) = 资源id低;  *(... + 0x2c + iVar8) = 资源id高; // u64 所有者
```
基址的另一入口：全局 `FUN_006373d0() + 0x54`（SC_cLotGroup.c:83、SC_cMetaLot.c:138/279）。

```
parcel[0x4C] 字段：
+0x10   i32  排队用的索引（指向 zone/lot 池）                     【中】
+0x14   u32  分区(zone)资源 id                                   【高】
+0x1c   u32  zone 类别位掩码（bit=RCI 类别；测试见 FUN_00852550:
             `(*(uint*)(this+100) & *(uint*)(idx*0x4c+0x1c+base)) != 0`）【高】
+0x20/+0x24 u64 已放置 lot（或建筑资源）id，-1=空                   【高】
+0x28/+0x2c u64 所有者/分区资源 id                                 【中】
解析 parcel → "cell 对象"用 `FUN_006d9960(base + idx*0x4c + 0x20)`（SC_cMetaLot.c:279），
其上字段：+0x0c u8 活动、+0x10 i32 道路 id、+0x38/+0x3c f32、+0xc8 i32 建筑资源 id 【中】
```

### 1.4 cZoningGame 字段表（SC_cZoningGame.c FUN_006f1200/006e1b10/006f1070/006e9900/006e39a0/006de3e0）【高】

```
+0x0c/+0x10     game/world 指针
+0x18/+0x1c     vec<int> 脏 zone 队列（对象侵入链 next 在 +0x68；逐个 FUN_0085da60 后清空）
+0x6c/+0x70     vec<int> 脏对象队列（侵入链 next 在 +0x5c；逐个 FUN_00863de0 后清空）
+0x74           上述队列游标
+0xd8/+0xdc     vec<u32> 每-lot 标志位（bit0 测试；FUN_006dffd0 复位）
+0xe8           **每-lot 大记录数组基址，步长 0x108**：
                  +0x10 i32 zone 类别索引（-1=未分区）
                  +0x50 u32 打包 RGBA 颜色
+0x818          ptr（事件时 *(ptr+0x2cc)=1）
+0x82c..0x833   脏标志组（0x82c=图需重建；0x831=工具打断；0x832=就绪）
+0x854/+0x858   vec<0x44> 分区桶：+0x00/+0x04 = 子向量 begin/end（lot id 列表）【高】
+0x864          u8 桶脏标志
+0x868/+0x86c、+0x878/+0x87c、+0x898/+0x89c  vec<u64> 待处理对（增删通知）
+0x8d8/+0x8dc   vec<u32> 每区颜色/计数
+0x8ec/+0x8f0/+0x8f4  hashmap（0x50B 结点：key u32, 17 个 u32 载荷, +0x4c u8, next@+0x50）【中】
+0x908/+0x90c   vec<0x88> zone 图形实例（+0x1c = zone id；刷新时逐个 FUN_006e83f0）【高】
+0x9a0/9a4/9a8、+0x9c0/9c4/9c8、+0x9e0/9e4/9e8  三个 hashmap
                  其中 +0x9c0 映射：lot 索引 → zone id（FUN_006f1200 内 `puVar7[1] = zoneElem+0x1c`）
+0xa0c..0xa24   4xu64（矩阵，FUN_006dbdc0）
+0xa2c/+0xa30   vec<int> 分区删除 pending（FUN_006e39a0 加、FUN_006e1d00 删）
+0xa3c/+0xa40   vec<u64> 分区添加 pending（u64 id 对）
+0xa4c          i32
+0xa70          子结构（=ToolMetaLotZone+0xa70）
+0xa74/0xa78/0xa7c  hashmap（zone 图形结点：key@0, next@+8, value@+4）
+0xa90/0xab0/0xad0  3x 矩阵，散列 0x2c7c7986 / 0xa2f9beeb / 0x2aa2e11c（zone 底色/蒙版/叠加纹理）
+0xafc          u32 打包 RGBA（属性 0xe93cc73 的 float4 *255, packssdw 饱和）
+0xb00/+0xb04   vec<0xC> zone 类别定义表（资源属性 0xe990a17，每项 3 个 i32）【高】
+0xb10          资源句柄（消息 0xe950425 写入）
```

### 1.5 关于"0x5C 步长 lot 大数组"线索的考证【高（结论修正）】

全库 `* 0x5c` 检索共 40+ 处，**全部指向传输网络（transport）结点数组，而非 lot 数组**：

- `GB_cTransportShared.c:374`（FUN_00c21800）：`iVar3 = param_3 * 0x5c + *(int*)(param_1+0x14)`，元素容量 `(*(param_1+0x18)-*(param_1+0x14))/0x5c`；写入 +0x3c/+0x40/+0x44（来自 param_2+0xb4/+0xb8/+0xbc 的位置 xyz）与 +0x54（ETA，`FUN_005ca440(res+0xa8)+res+0xa4`，负数截 0）。
- `SC_cTransportRail.c / SC_cTransportAirplane.c / SC_cTransportTraffic.c / SC_cTransportRadial.c / SC_cTransportPipe_cPool.c`：同模式；+0x30 与 +0x34 作链/占用比较（`*(int*)(*piVar2*0x5c+0x34+base) == param_2`）。
- `SC_cGameUI.c:1999`：以元素 +0x3c/+0x40/+0x44 做球距离拾取（最近站点），+0x34 经虚表 +0xC8(200) 解析为资源，其属性 0xe27b9af 得到过滤类型（与 0x2ea8fb98 比较）。
- `SC_cGraphicsDataView.c:56`：`iVar6*0x5c + piVar3[5]`（=基址+0x14），读 +0x30/+0x3c/+0x40/+0x44 —— 数据视图同源。
- `SC_cUnitPathCreationGlobalApplyWithZ.c:9`：`return iVar1 + 0x5c;` 只是"下一元素"步进。
- `SC_cZoningGame.c` FUN_006e1b10 中 `*(int*)(iVar2+0x5c)` 是侵入链表 next 字段偏移（配合 +0x6c 向量），不是数组步长。

**0x5C(92B) 元素还原**（传输站点/结点）【中】：
```
+0x30  i32 占用者/链 id          +0x34  u32 资源句柄（可解析，属性 0xe27b9af）
+0x3c/+0x40/+0x44 f32 位置 xyz    +0x54  f32 到站时间(ETA)
```
真正"lot 大数组"是 **cZoningGame+0xe8 的 0x108 步长记录数组**（§1.4）与 **0x4C parcel 数组**（§1.3）。原始线索中的 0x5C 应为混淆了 transport 站点数组或 +0x5c 链域。【高】

### 1.6 其余 stride 一览

| 步长 | 数组 | 字段证据 |
|---|---|---|
| 0x4C | parcel（§1.3） | SC_cZoningSplineGeoHelperFuncObj.c、SC_cLotGroup.c:83 |
| 0x108 | ZoningGame 每-lot 记录 | SC_cZoningGame.c FUN_006f1070（+0x10、+0x50） |
| 0x1B4 | cGraphicsZone 每-zone 图形元素 | +0x34 状态(0..3)、+0x13c/+0x140 引用计数材质句柄、+0x144/+0x17c uniform 偏移 |
| 0x144 | cLotBasedZoneHandler+0x14 每区上下文 | FUN_007e46d0 尾部循环 |
| 0x44 | zone 桶元素（ZoningGame+0x854、ToolMetaLotZone+0xb84） | +0/+4 子向量；+0x30 超时浮点（工具拖拽动画） |
| 0x88 | zone 图形实例（ZoningGame+0x908） | +0x1c zone id |
| 0xC | 候选建筑三元组（zoning系统+0x20 数组） | FUN_0085aa90/FUN_00859f10；取 [+8] 为建筑 id |
| 0x50 | cRoadManager 每路记录 | FUN_006cc540 |
| 0x38 | 每-lot 渲染行 / walker 入口 | FUN_007e13f0（步进 0x38）；walker +0x10 道路 id、+0x24 半径² |

---

## 2. 地块生成算法（沿路切分）

**核心模型：lot 不是网格，而是"道路 corner 对之间的一条带"**【高】
- lot 用两个 64 位 corner id（+0x84/0x88 与 +0x8c/0x90）定义，corner 同时被相邻 lot 共享。证据：cLotGroup FUN_00852550 直接拷贝两对 corner id；cMetaLot FUN_00852740 在聚合时若新 lot 的 corner A 等于已存 corner A，则把自身端点**延伸**到新 lot 的 corner B（街区连续生长）；cPlacementLotGroup FUN_008548b0 底部用 `FUN_0067cda0(vec, id)` 在链上查 corner 实现"沿路连续 lot 链"。
- **宽度/深度**：lot+0xa0/+0xa4 一对浮点（临路宽/进深），cMetaLot 聚合时取 max/min；放置判断 `FUN_00859110(宽度, 进深, zone, ...)`（walker 调用），过滤条件 `*(float*)(rec+4) <= *(float*)(lotRec+8)`（所需尺寸 ≤ lot 尺寸，FUN_007df640/007df680）【中】。
- **道路关联**：lot+0x10 = path/road id，-1 = 未接路。所有重叠/分区合并的判定先比道路 id（FUN_008640b0：`param_4+0x10 == param_2+0x10` 直接同路返回；再比 zone id）。
- **转角**：corner 对象与 lot 对象共用 +0x10（道路 id）字段（FUN_00854020 中 `FUN_006d99b0(cornerId)+0x10` 与 cell+0x10 比较"是否同一路"）；cPlacementLotGroup +0x68/+0x69 两个字节 = 两个端点 corner 的 `FUN_00857590()`（转角/端点判定标志）【低】。
- **spline 的作用**：
  - 划分区时，工具沿光标路径累积**样条点侵入链表**（ToolMetaLotZone +0xb08/+0xb0c，结点 +0x10 供 FUN_0083c690 画线），拖动距离 `sqrt(dx²+dz²)/param_4` 换算成段数（FUN_007debe0 开头）——即一笔涂抹被离散为沿线段。
  - parcel 创建 FUN_0085aaf0 调 `FUN_0085cd70(pts..., this+4)` 把 4 个参数的几何（两个 `FUN_0085ce0` 变换点）写入 parcel——**parcel 几何由道路 spline 上的两个点定义**【中】。
  - 拆除侧 `SC_cDestructionSplineGeoFuncObj.c / SC_cWholeDestructionSplineGeoFuncObj.c`（FUN_008d7630/008d76b0）：`FUN_008d5ce0(四参)` 构造带姿态的样条点，`FUN_008d7360/008d7520(prev,next,...)` 在链表中插入段并把新段 id 写入 `*(idArray + 0x28 + idx*0x40)`——整条街拆除时按原样条重建 lot 边界做几何收尾【中】。

**结论：SimCity2013 的地块切分 = 道路 corner 图 + 相邻 corner 间的可分带；玩家画 zone 时按道路两侧 parcel（0x4C 数组）逐格授权，lot 在 corner 间动态伸展/收缩。**

---

## 3. 分区机制（RCI 如何影响成长）

- **zone 类别 = 位掩码**【高】：parcel+0x1c 是类别位掩码；cLotGroup/cMetaLot 用工具的类别掩码（this+100 / +0x19）去测试/清除。创建 parcel 时初始写 4（bit2）再由 `FUN_0085d800()` 重写为当前类别。
- **zone 定义资源属性**（ToolMetaLotZone 构造 FUN_007dcd40 读入）【中】：
  - 0xdca011d / 0xdca012d：两套颜色（默认值在 DAT_00da307c，可被属性覆盖）→ 地面着色；
  - 0xdbb7cb2：另一颜色（DAT_00d1f318/DAT_00d241f8 默认）；
  - 0x9769ccd：整数模式（值 1 时 FUN_007debe0 走"拖拽持续"分支 +0xaec==1）；
  - 0xdcde809 / 0xdd1cefb / 0xdd1cf01 / 0xdd1cf0b：四个浮点（宽度/进深/间距参数，数值证据：FUN_007debe0 用 +0xae0 触发音效句柄、+0xad4 决定是否允许 FUN_007dde10 预览）【低】；
  - 0xe72d8eb、0x96cb206c：两个"存在性"开关 → +0xa61/+0xa62（0xa61 开启时还比对 0xf90e380/0xf90e381 名单与当前地图名 `FUN_00a0e940()`——**分区类型可限定地图/资源黑白名单**）【中】；
  - 0x2aa2e11c / 0xa2f9beeb / 0x2c7c7986（ZoningGame 载入）：三张变换/纹理矩阵。
- **渲染状态 0..3**（cGraphicsZone 元素+0x34）由三个勾选 +0x38/+0x39/+0x3a 控制：状态 2↔+0x38、1/3↔+0x39、0↔+0x3a。结合字符串 `s_GraphicsZone_draw_lots`，这是"按 RCI 类别过滤显示 lot 底色"的调试/覆盖开关；uniform 从 +0x114 浮点向量按 `elem+0x144` 或 `+0x17c` 取【中】。
- **密度升级条件**（FUN_008a6320/008a6290/008a6720，被 ZoneTool、AddTowerLevel、DefaultTool 共用）【高】：
  ```c
  mapRes = 世界查询(...)+0x210;
  level  = 属性(0x933277bb);            // 当前密度等级/档位
  cnt    = 属性(0xa1c4a200);  cap = 属性(0x9f5b9909);
  if (cnt+8 <= cap+4) return -1;         // 满 → 不能升级
  if (0 < cnt+4)      return 1;          // 有存量 → 可升级
  // FUN_008a6720: 属性 0xbf5d2e3 (f32 阈值) > 当前值 FUN_00606340(...)+4 → 达标
  ```
  即：**升级 = 需求值超过 0xbf5d2e3 阈值 且 数量未达 0x9f5b9909 上限**；0xbf5d2e5 是"可升级"开关属性，0xbf5d2e4 是另一档阈值，0xe531ef8 返回等级显示值。

---

## 4. 成长/开发循环（demand → build → upgrade 证据链）

1. **画 zone**：cToolMetaLotZone 拖拽 → FUN_007debe0 选目标 path part（FUN_007dd320/FUN_007dd6a0）→ FUN_007deaa0/007deb00 提交；或 FUN_00854020（MetaLot 重分区）：对半径内 parcel，若 `cell 道路 id 相同 且 已 zoned(+0xc)`，且旧建筑资源 `cell+0xc8 != 新zone资源`，则 `FUN_0083c610/FUN_00850ea0/FUN_0083c640(旧资源,0)` **拆除旧建筑并重新授权** —— 即"改分区=先清场"。【高】
2. **授权落盘**：FUN_0085aaf0 创建/更新 parcel（+0x1c 类别掩码，+0x20 建筑位=-1 空）；walker 接受时 `FUN_006d9ba0(lot+0x94, zoneRes)` 写 zone id（FUN_007e2b60）。【高】
3. **脏标记与合批**：所有变更置 cZoningGame+0x82c 脏位；增删进 pending 向量（+0xa2c/+0xa3c），每帧 FUN_006f3e20 里 `FUN_006e2630()/FUN_006f3b60()/FUN_006f3950()` 冲账；lot 被移除时 FUN_006de3e0 给邻居 cell 置 +0x11 脏字节（触发重新分区传播）。【高】
4. **开发（build）**：cLotBasedZoneHandler FUN_007e46d0【高】：
   - 先 `FUN_0085ffc0(lot, ctx+0x14)` 开事务；QueryPerformanceCounter/rdtsc 记时到 +0x2d8/+0x2dc，预算在 +0x2f0（f32）；`FUN_008e1ec0() > 预算 → *param_4 = -1`（**本帧跳过，留下帧** —— GlassBox 分帧成长）；+0x2cc 总开关。
   - 每 lot：`FUN_006d7680(lotId)` 取记录，候选表 = `rec+0x38..0x3c`（0xC 步长三元组），`FUN_00923480(count)` **随机选一个**，取 `entry+8` 为建筑资源 id；表空且 rec+0x2c 为 0 则跳过。
   - `FUN_007e1200(选中建筑, lot)` 应用；`FUN_007e3fb0(...)` 校验放置；失败 → 组装默认规格 `{0.0, 1.0, 1, &DAT_00e12d88}` 并 `FUN_0085f930(lot, ..., ...)` —— **放占位/废弃建筑兜底**（配套 `SC_SC_UcShaderDataAbandonedBuilding` 材质数据存在）；成功 → `FUN_007e4510(rec, i)`。
   - 尾部遍历 +0x14/+0x18 的 0x144 元素数组调 FUN_00851490 —— 按区刷新。 [注：候选表 0xC 三元组在 FUN_0085aa90 处 `{u64 id, u32}` 写入 —— 即“该 zone 允许的建筑变体清单”，+8 处为资源 id。]
5. **upgrade**：AddTowerLevel 工具与自动成长共用 §3 密度阈值查询；ZoningGame FUN_006e9900 把 lot id 挂进 +0x854 的 0x44 桶（桶空则直接写，非空 FUN_006051a0 追加）——**按区批量入队升级**。【中】
6. **候选来源（demand 侧）**：本转储集中**没有**直接的 RCI demand 计算文件（grep "demand" 只命中网络下载字符串）；demand→候选表的填充发生在未转储模块。但 FUN_008a6720 的"阈值比较"与 FUN_00864010 的"zone id 比较合并"给出了需求侧判定接口。**结论：demand 计算（RCI 数值）不在本批文件中；build 侧证据完整。**【高（就本库而言）】

---

## 5. 与 LOT property（0x00B1B104 已解析部分）的关系

已解析的 LOT property 字段与引擎消费点对应关系【中，字段名侧为推测】：

| 我们解析的 LOT 字段 | 引擎消费证据 |
|---|---|
| **LotSize（宽度/进深）** | lot+0xa0/+0xa4 宽度对参与 cMetaLot 聚合；walker 过滤 `所需 <= lot 尺寸`（FUN_007df640）；FUN_00859110 的第 1/2 参数即宽/深适配检查 |
| **LotMask** | 对应 parcel+0x1c 类别位掩码体系：lot 只能落在掩码匹配的 parcel 上（FUN_00852550 用 zone 掩码过滤 cell；FUN_00852740 清掩码外 parcel）。LOT 级别的 mask 决定该建筑允许的 zone 类别/坡向【中】 |
| **LOD 模型引用（资源 id）** | 建筑资源 id 走 64 位 id 通道：cell+0xc8（现存建筑）、parcel+0x20（已放置）、FUN_0085aaf0 处 -1 初始化；渲染经 `vtable+0x128(lotId)` → 每-lot 图形记录（FUN_007e13f0，0x38B 渲染行）与 0x108 记录 +0x50 颜色。LOD 选择本身在渲染侧（SC::UcShaderData LotInstanceInfo 材质数据） |
| 颜色/类别 | 0x108 记录 +0x10 类别索引与 +0x50 RGBA：引擎给每个 lot 按其 zone 类别查 +0xb00（0xC×3 类别表，属性 0xe990a17）取色绘制（FUN_006f1070 消息 2：FUN_007e2260 按 `图形记录+0x2a8` 的类别上色） |

即：**LOT property 描述"单体建筑地皮"静态规格；引擎在 parcel（0x4C）上记录动态状态（zone 掩码、已放 lot、类别位），在 0x108 记录上记录着色/类别，成长时按 zone 从候选表随机挑 LOT 资源并校验 LotSize。**

---

## 6. 对 OpenSCP 的可复用结论

1. **数据模型**（置信度高，可直接照抄）：`Corner ↔ Lot(cornerA,cornerB,roadId,zoneId) ↔ Parcel(classMask, lotRef, buildingRes)` 三层；parcel 定长 0x4C。实现时用 `u64 id + 池 + 句柄解析`（对应 FUN_006d9960）。
2. **切分算法**：不要用网格；以道路 corner 图为骨架，lot=相邻 corner 间的带（宽 0xa0/进深 0xa4），转角处 corner 复用 +0x10 道路 id 判定。spline 仅用于拖拽离散化与拆除几何重建。
3. **成长循环**：把"选建筑"做成 `zone → 候选三元组表（0xC：id 变体）`，每帧对每个空/待升级 parcel 做带时间预算（f32，超时 -1 顺延）的随机抽取 + 放置校验 + 失败落废弃兜底。这是本引擎手感（慢慢长、不卡帧）的关键。
4. **密度升级**：`需求值 > 阈值(0xbf5d2e3) 且 count < cap(0x9f5b9909) 且 开关(0xbf5d2e5)`；等级存属性 0x933277bb。
5. **改分区语义**：对已建成 parcel 改 zone = 拆旧建筑（资源卸载三连 FUN_0083c610/00850ea0/0083c640）+ 掩码重写 + 邻居脏传播（+0x11）。
6. **警示**：网上流传的"0x5C 步长 lot 数组"应更正为 transport 站点数组；lot 主表步长是 0x108（cZoningGame+0xe8），parcel 是 0x4C。【高】

### 主要证据文件（绝对路径）
- `D:\rust\packages\fluffy-open-scp\docs\source-code\SC_cZoningSplineGeoHelperFuncObj.c`（FUN_0085aaf0 parcel 创建）
- `D:\rust\packages\fluffy-open-scp\docs\source-code\SC_cLotBasedZoneHandler.c`（FUN_007e46d0 开发循环、FUN_007e13f0 渲染行）
- `D:\rust\packages\fluffy-open-scp\docs\source-code\SC_cZoningGame.c`（载入/0x108 表/pending/0x44 桶）
- `D:\rust\packages\fluffy-open-scp\docs\source-code\SC_cToolMetaLotZone.c`（工具字段 0xa5c..0xb88、属性散列、密度助手 FUN_008a6xxx）
- `D:\rust\packages\fluffy-open-scp\docs\source-code\SC_cMetaLot.c` / `SC_cLotGroup.c` / `SC_cPlacementLotGroup.c`（聚合与 corner 链）
- `D:\rust\packages\fluffy-open-scp\docs\source-code\SC_cGraphicsZone.c`（0x1B4 元素、draw_lots 字符串）
- `D:\rust\packages\fluffy-open-scp\docs\source-code\GB_cTransportShared.c`、`SC_cTransportRail.c` 等（0x5C 数组真身）
- `D:\rust\packages\fluffy-open-scp\docs\source-code\SC_cFirstZonedOverlapper.c`、`SC_cConflictingZonedOverlapper.c`、`SC_cUnzonedWithZonedOverlapper.c`、`SC_cLotsShareRoadAndZone.c`（重叠/同路同区过滤链）