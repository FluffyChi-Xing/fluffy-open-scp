# RenderWare 3.7.0.2 官方源码结构地图与 RW4 写出对照（2026-10-31）

来源：`D:\source-map\rwsrc-v3.7.0.2`（Criterion 官方 C 源码，PC 裁剪版：仅
d3d8/d3d9 驱动，233 头文件 + 134 C 文件；skin/matfx 等部分插件只有闭源
`misc/lib/*.lib`）。本文把官方写出器的**权威格式事实**提炼出来，供
`crates/rw4` 写出路径对拍，替代小包盲测。所有行号均指该源码树。

## 一、源码树结构

| 目录 | 角色 |
|---|---|
| `core/src/plcore/` | **核心流格式**：chunk 头、版本戳、插件注册、EXTENSION 读写、chunk ID 总表（`batype.h`） |
| `core/src/` | 核心对象：raster(`baraster`)、texture(`batextur`)、texture dict 序列化(`babintex`)、image/raster 互转(`baimras`)、重采样(`baresamp`)、camera/frame/device |
| `core/src/pipe/p2/` | **PowerPipe**（p2）数据流管线架构 + 标准簇定义（`p2stdcls.h`） |
| `core/src/pipe/p2/d3d9/` | D3D9 IM3D 顶点模型、提交节点 |
| `core/driver/d3d9/`（及 d3d8） | **平台驱动**：raster 格式映射、native 纹理字典（`d3d9texdic.c`）、顶点声明/shader 组装、灯光 |
| `world/` | world 插件：clump/atomic/geometry/material/matlist/mesh/**meshop**/sector/world 的运行时 + **序列化**（`babinwor.c`） |
| `world/pipe/p2/d3d9/` | **D3D9 渲染主路径**：AtomicAllInOne 节点、resEntry 实例化数据、顶点声明构建、VS 汇编拼装、native 几何写出（`native.c`） |
| `collis/` | 碰撞插件（源码完整）：CollTree 分割树 + 二进制格式 |
| `toon/` | 卡通渲染插件（FX pack，源码完整，含已汇编 D3D9 shader） |
| `misc/inc/` | SDK 汇总头：全部 rp*.h（插件）/rt*.h（工具包）+ `rwcore.h`/`rwplcore.h`；`.rpe` = 插件导出描述（动态加载用） |
| `misc/lib/` | 闭源插件 `.lib`（**rpskin、rpmatfx 的实现体在此，源码树缺失**） |

## 二、流格式核心事实（写出的根）

1. **chunk 头 = 12 字节小端**：`{type:u32, length:u32, libraryID:u32}`，
   `rwCHUNKHEADERSIZE = 12`（`core/src/plcore/babinary.h:23-25`）。
2. **版本戳打包**（`rwversion.h:90-100`）：
   `libraryID = (((version - 0x30000) & 0x3ff00) << 14) | ((version & 0x3f) << 16) | (buildNum & 0xffff)`；
   version 编码 `0xVJNBB` = V<<16 | J<<12 | N<<8 | BB，3.7.0.2 = **0x37002**。
   读取端：`libraryID` 高 16 位为 0 时按旧式 `version = libraryID << 8` 解析
   （`babinary.c:276-289`）。写侧恒用 `rwLIBRARYCURRENTVERSION + RWBUILDNUMBER`
   （`babinary.h:106-108`）。
3. **chunk ID = (vendor<<8) | id**：`MAKECHUNKID(vendorID, chunkID) =
   ((vendorID & 0xFFFFFF) << 8) | (chunkID & 0xFF)`（`batype.h:31`）。
   vendor：CORE=0、CRITERIONTK=1、CRITERIONWORLD=5（`batype.h:48-56`）。
   core chunk 表：STRUCT=0x100, STRING=0x200, EXTENSION=0x300, TEXTURE=0x600,
   MATERIAL=0x700, MATLIST=0x800, WORLD=0xB00, FRAMELIST=0xE00, GEOMETRY=0xF00,
   CLUMP=0x1000, LIGHT=0x1200, ATOMIC=0x1400, TEXTURENATIVE=0x1500,
   TEXDICTIONARY=0x1600, GEOMETRYLIST=0x1A00, RIGHTTORENDER=0x1F00,
   TOC=0x2400, UVANIMDICT=0x2B00, COLLTREE=0x2C00（`batype.h:94-209`）。
   Criterion 插件 chunk：SKIN=0x116, HANIM=0x11E, COLL=0x11D,
   MATERIALEFFECTS=0x120, UVANIM=0x135, TOON=0x12E（`misc/inc/rpcriter.h`）。
4. **EXTENSION 包裹块机制**（`batkbin.c:436-479`，写出路径原文）：
   每个可流对象末尾**必写**一个 `rwID_EXTENSION` 头（size=全部插件数据总大小，
   可为 0），然后按插件注册顺序逐个写 `[插件 chunk 头 + 数据]`；
   `getSizeCB <= 0` 的插件跳过。读取端按 length 循环 `(readLength + 12)` 消耗
   （`batkbin.c:226-282`），未注册插件**跳过但不失败**。
5. **版本门控模式**：读侧 STRUCT chunk version 必须落在
   `[rwLIBRARYBASEVERSION=0x35000, rwLIBRARYCURRENTVERSION]`，否则 E_RW_BADVERSION
   直接失败；结构演化靠 STRUCT **size** 区分（如 world 的 bbox 是 34003 后才有，
   `babinwor.c:1574-1580`）。**写侧无版本分支**——恒写最新布局。
6. struct 块写前统一 `RwMemLittleEndian32 + RwMemRealToFloat32`（小端、float32）。

## 三、几何/材质/网格 序列化布局（world/）

### 3.1 GEOMETRY chunk（`world/bageomet.c:2687-2891`）

```
rwID_GEOMETRY 头(size=总大小)
├─ rwID_STRUCT 头(16B)   ← RpGeometryChunkInfo (bageomet.h:210-218)
│   format:i32 = flags低8位 | TEXCOORDSETS(n)=(n&0xff)<<16   (bageomet.c:2749)
│   numTriangles:i32, numVertices:i32, numMorphTargets:i32
│   （!NATIVE 且 numVertices>0 时接数据，顺序固定）
│   prelit    : numVertices × 4×u8      （若 flags & PRELIT）
│   texCoords : n套 × numVertices × 2×f32
│   triangles : 每个 8B 两条 u32：
│       vertex01   = v0<<16 | v1          (bageomet.c:78-84, 2800-2825)
│       vertex2Mat = v2<<16 | matIndex
│   每 morph target (bageomet.c:2829-2875)：
│       {boundingSphere(center:3f,radius:f), pointsPresent:i32, normalsPresent:i32}
│       + 顶点 f32×3×N（present 时）+ 法线 f32×3×N（present 时）
│   ——注意：法线属于 morph target，不在 geometry 顶层
├─ rwID_MATLIST chunk（见 3.3）
└─ rwID_EXTENSION（含 BINMESHPLUGIN / NATIVEDATAPLUGIN / RIGHTTORENDER）
```

format 位（`bageomet.h:51-88`）：TRISTRIP=0x01, POSITIONS=0x02, TEXTURED=0x04,
PRELIT=0x08, NORMALS=0x10, LIGHT=0x20, MODULATE=0x40, TEXTURED2=0x80,
NATIVE=0x01000000；texcoord 套数在 bits16-23，为 0 时回落 TEXTURED2→2/TEXTURED→1
（`bageomet.c:127-130`）。

### 3.2 MATERIAL / MATLIST

- MATERIAL STRUCT 恒 28B（`bamateri.h:49-57`，`bamateri.c:1344-1359`）：
  `flags:i32(恒0) → color:RGBA 4×u8 → unused:i32 → textured:bool(=texture!=NULL)
  → surfaceProps 3×f32`。**surfaceProps 顺序 = ambient, specular, diffuse**
  （`batypes.h:560-565`——与文档措辞"ambient,diffuse,specular"不同，是写出陷阱）。
- MATLIST（`bamatlst.c:427-492`）：STRUCT = numMaterials:i32 +
  n×i32 引用表（第 i 项 = 最大的 j<i 使材质重复，否则负数）；然后仅对"首次
  出现"的材质内嵌完整 MATERIAL chunk。**重复材质不重复写**。

### 3.3 BINMESH 扩展块（`world/bamesh.c:540-634`）

```
binMeshHeader { flags:u32, numMeshes:u32, totalIndicesInMesh:u32 }   (12B)
每 mesh { numIndices:u32, matIndex:i32 } + [!NATIVE 时] numIndices × u32 索引
```
- flags 位（`bamesh.h:85-107`）：TRISTRIP=0x0001, TRIFAN=0x0002, LINELIST=0x0004,
  POINTLIST=0x0010, PRIMMASK=0xFF, UNINDEXED=0x0100；**trilist = 位全 0**。
- `matIndex` 按 geometry 的 matList 编号，找不到**强制写 0**（`bamesh.c:578-584`）。
- 内存态 `serialNum`/`firstMeshOffset` **不写文件**（二进制头只有 3 字段）。
- **写前硬约束**：顶点必须按 mesh 材质分组、材质边界复制顶点，各 mesh 顶点区间
  连续不重叠——RWDEBUG 下 `RpGeometryStreamWrite` 直接报
  `E_RX_MESHES_RANGES_OVERLAP` 失败（`bageomet.c:2701-2726`）；游戏读侧
  mesh→draw-range 同样假设此布局。
- tristrip 生成（`bameshop.c`）：strip 间用 2 个重复索引衔接，winding 错时插 3
  索引翻转（`bameshop.c:3243-3247`）——所以 `totalIndices > 3×tris` 正常；
  **winding 恒定一致**。透明判定 = 纹理含 alpha 或材质 color.alpha≠0xFF，排序
  键 `transparent > raster > pipeline > texture > mat`（`bameshop.c:203-304`）。

### 3.4 CLUMP（`baclump.c:4332-4437`）

STRUCT(12B)=numAtomics/numLights/numCameras → FRAMELIST（frame[0]=clump 根）
→ GEOMETRYLIST（STRUCT 4B + n + 内嵌完整 GEOMETRY chunk ×n）→ 每 ATOMIC 一个
chunk（STRUCT 16B = frameIndex/geomIndex/flags/unused，共享 geometry 时
geomIndex 指向列表，无内嵌 geometry）→ EXTENSION。

## 四、纹理 / Raster（core/src + d3d9 驱动）

1. **Raster 格式编码**（`baraster.h:125-151`，`cFormat = format>>8`）：
   1555=0x100, 565=0x200, 4444=0x300, LUM8=0x400, 8888=0x500, 888=0x600,
   16=0x700, 24=0x800, 32=0x900, 555=0xA00；**附加位** AUTOMIPMAP=0x1000,
   PAL8=0x2000, PAL4=0x4000, MIPMAP=0x8000。**无 8888A 枚举、无 DXT 枚举**——
   DXT 由 D3D fourcc 承载。
2. **RwRaster 结构**（`baraster.h:190-207`，56B）：parent/cpPixels/palette/
   width/height/depth/stride/nOffsetXY/cType/cFlags/privateFlags/cFormat/
   original{Pixels,Width,Height,Stride}。类型位 NORMAL=0, ZBUFFER=1, CAMERA=2,
   TEXTURE=4（低 3 位）；DONTALLOCATE=0x80。
3. **filterAddressing u32**（`batextur.h:44-48`，注释
   `[xxxxxxxx xxxxxxxx vvvvuuuu ffffffff]`）：bits0-7=filter
   （NA=0,NEAREST=1,LINEAR=2,MIPNEAREST=3,MIPLINEAR=4,LINEARMIPNEAREST=5,
   LINEARMIPLINEAR=6，`baimmedi.h:229-240`）；bits8-11=addressU、bits12-15=addressV
   （WRAP=1,MIRROR=2,CLAMP=3,BORDER=4，`baimmedi.h:308-317`）。
   序列化时 bits16-23 再打包 streamFlags（USERMIPMAPS=0x01，`babintex.c:249-264`）。
4. **TEXDICTIONARY**（`babintex.c:754-830`）：STRUCT = `{numTextures:u16,
   deviceId:u16}` → 每纹理一个 `rwID_TEXTURENATIVE` chunk。
5. **D3D9 native 纹理**（`d3d9texdic.c:48-85`）：头 72B = `{id:i32(=9 PCD3D9),
   filterAndAddress:i32, name[32], mask[32]}` + raster 16B =
   `{format:u32(RW格式位), d3dFormat:u32(含 DXT fourcc 0xC1-0xC5), width:u16,
   height:u16, depth:u8, numMipLevels:u8, type:u8, flags:u8(HAS_ALPHA=1,
   IS_CUBE=2, USE_AUTOMIPMAPGEN=4, IS_COMPRESSED=8)}` → palette
   (PAL4=32 项/PAL8=256 项 ×4B PALETTEENTRY) → **每 mip 一个 `{size:u32, data}`**；
   DXT mip 尺寸 = `height<4 ? Pitch : (height/4)*Pitch`（`:401-415`）。
   平台 ID 枚举（`batype.h:288-301`）：D3D8=8, **D3D9=9**, PSP=10…
6. **RW→D3D 格式映射表**（`d3d9raster.c:52-65`）：1555→A1R5G5B5, 565→R5G6B5,
   4444→A4R4G4B4, LUM8→L8, 8888→A8R8G8B8, 888→X8R8G8B8, 555→X1R5G5B5。
   8888 的 alpha 有无由该表 alpha 列表达（无"8888A"这种 RW 格式名）。
7. palette 项字节序 = `{R,G,B,flags}`，与 RwRGBA 一致，直接 memcpy
   （`d3d9convrt.c:2186-2189`）。
8. mip 排布：native 文件按 level0..N 独立成块（非打包连续）；级数 =
   `max(w,h)` 不断右移（`d3d9raster.c:4042-4073`）；POW2 caps 下尺寸向下取
   2 的幂（`d3d9convrt.c:113-163`）。

## 五、渲染实现（管线与 D3D9 主路径）

1. **p2 = PowerPipe**：三类对象管线（Atomic / WorldSector / Material），
   `RxPipeline` = 节点拓扑排序数组；执行是直接递归调 nodeBody
   （`p2core.h:911-946, 1182-1211`）。标准簇 `p2stdcls.h`：
   `RxObjSpace3DVertex{pos,normal,color(ARGB打包),u,v}`、
   `RxGeometryFlag` 与 geometry format 位一一对应。
2. **D3D9 主路径 = AllInOne 单节点**（`world/pipe/p2/d3d9/nodeD3D9AtomicAllInOne.c:1037-1260`）：
   无 native 数据则 instance → 缓存 `RxD3D9ResEntry`
   （serialNumber 变化强制重 instance）→ 渲染回调逐 mesh
   `DrawIndexedPrimitive(primType, baseIndex, 0, numVertices, startIndex, numPrimitives)`。
3. **实例化数据布局**（`D3D9pipe.h:47-100`，RW3 native 几何段、RW4 mesh 的前身）：
   `ResEntryHeader{serialNumber, numMeshes, indexBuffer, primType,
   vertexStream[2]{vertexBuffer,offset,stride,geometryFlags,managed,dynamicLock},
   useOffsets, vertexDeclaration, totalNumIndex, totalNumVertex}` +
   每 mesh `InstanceData{numIndex, minVert, material*, vertexAlpha, vertexShader,
   baseIndex, numVertices, startIndex, numPrimitives}`。
   → SC4 RW4 mesh 头（tri_section/tri_count/start_index/index_count/
   min_vertex/vertex_count/vertex_section）就是这组语义的容器化改写：
   **min_vertex/vertex_count 是 draw-range**，start_index 是共享索引池切片。
4. **顶点声明构建**（`nodeD3D9AtomicAllInOne.c:258-512`）：元素顺序
   POSITION(FLOAT3) → NORMAL(支持则 **DEC3N**，否则 FLOAT3) →
   COLOR(D3DCOLOR, prelit) → TEXCOORDn(FLOAT2) → 可选 TANGENT(DEC3N)；
   按 Stream→Offset 排序，尾元素 `{Stream=0xFF, Type=UNUSED}` 终止。
   → 与 RW4 VertexFormat(0x20004) 的 12B/元素 + 尾元素完全同构；
   `native.c:57-135` 的 `_rpGeometryNativeWrite` 把 `D3DVERTEXELEMENT9[20]`
   原样写进二进制——**RW4 VertexFormat section 的直系祖先**。
5. **法线量化**：world sector 法线 = int8×4，±1→±127（`basector.h:45-97`）；
   SC4 模型 NORMAL UBYTE4 解包 `(b-127.5)/127.5` 与同一约定。
   VS 管线时 DEC3N 有硬件支持优先（`DEC3N` = 10-10-10 符号归一化）。
6. **VS 着色器**（`d3d9vertexshader.c/.h`、`d3d9vertexshaderdefs.h:5-38`）：
   输入寄存器固定 v0=POSITION, v1=WEIGHTS, v2=INDICES, v3=NORMAL, v4=COLOR,
   v5..v12=TEXCOORD0..7, v13=TANGENT；常量 c0-3=world-view-proj, c4=ambient,
   c5+=灯光；每骨 3 常量。运行时文本拼 vs_1_1/2_0/3_0 汇编再 D3DXAssembleShader。
   effect 类型表（`d3d9vertexshader.h:29-46`）：BUMPMAP/ENVMAP/DUAL/UVANIM/
   NORMALMAP/CUBEMAP…与 matfx 枚举对应（`rpmatfx.h:35-49`：NULL=0,BUMP=1,
   ENV=2,BUMPENV=3,DUAL=4,UVTRANSFORM=5,DUALUVTRANSFORM=6）。
7. **COLLTREE**（`collis/ctdata.c:24-32,157-178`）：STRUCT =
   `{flags:u32, inf:3f, sup:3f, numEntries:u32, numSplits:u32}` →
   numSplits × `RpCollSplit{left,right}`，每 sector 12B =
   `{type:u8(负向位0x01+轴X/Y/Z=0x00/0x04/0x08), contents:u8, index:u16, value:f32}`；
   USEMAP(0x01) 时再写 numEntries × i16。树只引用外部三角形数组。
   → RW4 COLLISION_MESH 同源（SC4 引擎缺失时自建 KD-tree 缓存）。
8. **动画**：`RtAnimAnimation` 流 = numFrames+flags+duration+关键帧数组
   （`rtanim.h:249-287`）；骨骼标准关键帧 = time+f32 quat+trans（40B 对齐，
   prevFrame 指针不写流）；压缩关键帧 typeID=0x2，四元数/平移各 16bit，
   自定义半浮点解压公式在 `rtcmpkey.h:118-130`，流尾附
   `{offset:V3d, scalar:V3d}` 反量化对（`rtcmpkey.h:99-105`）。
   HAnim nodeInfo = `{nodeID,nodeIndex,flags}`，flags POP/PUSH=0x01/0x02。
9. **UV 动画**：customData = `{name[32], nodeToUVChannelMap[8]:u32, refCount:u32}`
   （`rpuvanim.h:96-103`，注释明确 "streaming depends on layout"）；
   线性关键帧 6 浮点 = right/up/pos 两行仿射（`rpuvanim.h:162-173`）。
10. **TOC 容器**（`rttoc.h:52-75`）：`{numEntries, entry[]{chunkId, gid,
    offset(自文件头), guid}}`——RW"按偏移随机访问的目录"，RW4 section index
    的思想原型。

## 六、对 `crates/rw4` 写出问题的对照检查表

游戏"不识别"（占位/资源加载失败）时，按官方写出器的硬约束逐条核：

1. **顶点按材质分组排序 + 边界复制**（RW3 硬约束，§3.3）。merged-single 全部
   三角形同 material 0 时天然满足；但若后续恢复多材质逐 mesh，必须重排顶点、
   材质边界复制，且 mesh 头 `min_vertex/vertex_count` 必须是该 mesh 实际
   顶点区间（draw-range 语义），与顶点池切片严格一致。
2. **三角形 winding 恒定**。GLB 导入时的面朝向要统一到引擎的剔除方向；
   原版普查文件可抽取 3-4 个三角形验证 (v0,v1,v2) 与法线的关系。
3. **mesh 头契约**：`index_count == 3 × triangle_count`（已遵守）；
   TA 头 `index_count` 必须等于该 TA 的真实索引总数（多 mesh 共享 TA 时
   为全部 mesh 之和，我们当前实现每 mesh 一条 TA——与原版"共享池+切片"
   不一致，需对拍原版是否出现"1 TA 多 mesh 切片"再定）。
4. **VertexFormat 尾元素**：确认写出时是否带 `{Stream=0xFF, Type=UNUSED}`
   终止元素（RW3.7 声明数组约定，native.c 同样写 20 项定长数组）；以及
   NORMAL 用 UBYTE4 还是 FLOAT3 必须与原版同模型的声明一致（普查：
   2037 UByte4 vs 231 Float3——**多数模型是压缩法线**，写出端同样要压缩）。
5. **材质 28B struct 字段顺序**（RW3）→ RW4 material section 已按 C# 对齐，
   但 surfaceProps 的 (ambient,specular,diffuse) 顺序陷阱值得复查 RW4 侧
   字段对应。
6. **Raster section**：2×2 占位 raster 的 format 字段要用 RW 编码（8888=0x500
   或 MIPMAP 位等）而非 D3D 枚举值（若原版占位用的就是 RW 编码）。
7. **BBox = placement 后几何的 tight box**（census 已遵守）。
8. **头 word25（0xC0XXXX）**：RW3.7 中无对应字段，疑为 RW4 流系统元数据；
   已克隆 NightHouse 值，保持对拍策略。
9. **EXTENSION 式"总在最末"的心智**：RW3 每个对象必须收尾一个 EXTENSION 块；
   RW4 里对应语义是 MMA(0x2001A)/handles(0xFF0000) 等 section 的**存在与
   顺序**——原版 census 的 section 序列模板是唯一权威，新 section 只允许
   在已验证锚点插入（"只追加不重排"决策正确）。

## 七、缺口声明

- **skin / matfx 的 .c 实现不在源码树**（`misc/lib/rpskin.lib` 等闭源）；
  蒙皮 plugin data（bone 分组、权重/索引写出顺序）只能引用 `rpskin.h` 的
  权重排序约定（有效权重在前，0 权重殿后，`rpskin.h:34-47`）。
- RW4 容器层（magic/section index/fixup/0x10004-8）与 RW3.7 流完全不同，
  以 `ReadResourceArena_1`/`FixupStart`（ida）+ 原版普查为准；本文 RW3.7
  知识用于**section 内容语义**与**写出硬约束**，不覆盖容器布局。
- d3d8 系 native 纹理用 `dxtFormat` 字节（1-5），d3d9 系用 `d3dFormat`
  fourcc——两套并存的历史在排查 RW4 raster 字段时都要想到。
