# SimCity 资源文件类型简介

SimCity (2013) 的所有游戏数据都打包在 `.package` 文件（DBPF 容器）中，每个资源由三元组 **TGI**（Type / Group / Instance）唯一标识。下表列出 OpenSCP 目前已解析/可预览的全部文件类型，及其二进制特征；表格之后按类型分章节给出详情。

## 已解析类型总表

| 类型 ID | 名称 | 二进制特征 | 解析能力 |
|---|---|---|---|
| `0x00B1B104` | Property | 二进制属性表（hash + 类型 + 值行结构，无文件魔数） | 完整解析（含 canonical 写回）、Lot/Unit 装配 |
| `0x2F4E681B` | RW4 | RenderWare4 容器：header + section index + Blob | Mesh/材质/贴图/骨骼/动画解码，GLB/OBJ 导出 |
| `0x2F4E681C` | Raster | 裸像素（无头；LotMask 为 raw RGBA） | 解码 + PNG 化预览 |
| `0x2F7D0004` | PNG | `89 50 4E 47` | 直接预览 |
| `0x2F7D0006` | TGA | TGA 头（footer 可含 `TRUEVISION-XFILE`） | 解码为 PNG 预览 |
| `0x2F7D0007` | GIF | `47 49 46 38` | 直接预览 |
| `0x3F8662EA` | JPG | `FF D8 FF` | 直接预览 |
| `0x02393756` | Cursor | ICO/CUR 容器（`00 00 01 00`） | 解码为 PNG 预览 |
| `0x03E421EC` | Terrain Field Map (8-bit) | 20B 大端头 `[0,w,h,1,byte_count]` + 灰度像素 | 解码预览 |
| `0x03E421ED` | Terrain Field Map (32-bit) | 同上，channel_code=2，实为 RGBA | 解码预览 |
| `0x03E421F0` | Terrain Heightmap (16-bit) | 同上，channel_code=7，大端 u16 单通道 | 解码预览 |
| `0x0A98EAF0` | Locale JSON | UTF-8 BOM + JSON 文本 | 文本预览（剥离 BOM） |
| `0x67771F5C` | JavaScript | 纯文本 | 文本预览（语法高亮） |
| `0x2C978DB6` | CSS | 纯文本 | 文本预览 |
| `0xDD6233D6` | HTML | 纯文本（`<` 开头） | 文本预览 |
| `0x0469A3F7` | Shader 源 | HLSL 纯文本 | 文本预览（cpp 高亮） |
| `0x024A0E52` | State Script | 纯文本（`#` 注释 + `state`/`mode` 关键字） | 文本预览（ini 高亮） |
| `0x0D9E5710` | Wwise 音频 | `OggS` / RIFF-WAVE（Wwise 变体） | 播放预览（vgmstream） |
| `0x0A4D8D09` | Wwise Bank | `BKHD` 魔数 | 结构识别 + 播放 |
| `0x376840D7` | VP60 视频 | VP6 流（`VP6`/FLV 系头） | 视频预览 |
| `0x276CA4B9` | TrueType Font | `00 01 00 00`（TTF） | 字体预览 |
| `0xEA5118B0` | Effects Directory | 效果目录二进制表 | 结构识别 |
| `0x08068AEB` | ER2 Binary Rule | ER2 规则二进制 | 结构识别（未解析） |
| `0x08068AAC` | ER2 Rule | ER2 规则文本变体 | 结构识别（未解析） |
| `0x08068AED` | EcoGame State Data | `1F 8B` gzip；展开后与离线存档 .egb 同魔数（`62 2b 9c d7` @+4），EcoGame 状态快照 | hex 预览（未解析） |
| `0x08068AEE` | EcoGame State Table | 12 字节记录表（序号 + 位模式字段） | hex 预览（未解析） |
| — | DDS（RW4 内嵌） | `44 44 53 20`（"DDS "） | DXT1/5 解码导出 |
| — | SQLite 注册表 | `53 51 4C 69 74 65`（database_main.s3db） | FileTypes/Instances 查询 |

## 分章节详情

### Property（0x00B1B104）

数据驱动核心：引擎只解释属性，行为语义全部在数据侧。每条属性由 **hash 标识符 + 数据类型 + 值** 构成；跨资源引用以 **Key**（TGI 三元组）类型存储，跨 package 生效。

#### Lot 定位四件套与模型引用

| 标识符 | 名称 | 类型 | 说明 |
|---|---|---|---|
| `0x00F9EFBB`–`0x00F9EFBE` | UnitLOD1–4 | Key | 四级模型引用（指向 RW4） |
| `0x0CCB7FC8` | LotSize | Float2（vec2） | 地面尺寸（米） |
| `0x0CCB7FC9` | LotOverlayOffset | Float2 | 地面覆盖偏移 |
| `0x0CCB7FD5` | LotMask | Key | 四通道地面掩码（指向 Raster） |
| `0x0DB7FB17` | LotPlacementTransform | Transform（12 floats，行主序） | 模型↔地块定位；地面取其逆 |
| `0x0D02D586`–`0x0D02D589` | LotColor1–4 | 颜色值（RGBA） | 通道着色；A = 地面贴图索引 0–15 |
| `0x0CCB7FD4` | Lot Textures | Key | 纯纹理容器（如 1024² DXT5 地面图） |

#### 灯光（Light Unit）

| 标识符 | 名称 | 类型 |
|---|---|---|
| `0x0CAA8F10` | LightIDs | Int32 列 |
| `0x0CAA8F11` | LightTypes | Key 列（Point `0x75D4C8CD` / Spot `0x2F0FF9FD` / Line `0x0820ABAF`） |
| `0x0CAA8F12` | LightTransforms | Transform 列（行主序 12 floats） |
| `0x0CAA8F13` | LightColors | Float3 列（RGB 0–1） |
| `0x0CAA8F14` / `0x0CAA8F15` | Radii / InnerRadii | Float 列 |
| `0x0CAA8F16` | DiffuseLevels | Float 列 |
| `0x0CAA8F17` | DebugNames | String 列 |
| `0x0D4C96B4` | SpecLevels | Float 列 |
| `0x0D4CAD23` | Lengths | Float 列（Spot/Line） |
| `0x0E01200F` | CullDistances | Float 列 |
| `0x0E4DF244` | FalloffStarts | Float 列 |
| `0x0E98D445` / `0x0EC4567E` | IsVolumetric / VolStrengths | Bool / Float 列 |

#### 效果 / 贴花 / 道具 / 路径 / 生成器

| 标识符 | 名称 | 类型 | 说明 |
|---|---|---|---|
| `0x02A907B5`–`0x02A907BC` | Effect IDs/Transforms/AlwaysZero/RefIDs/Enabled | Key/Transform/Int32/Key/Bool 列 | 效果单元五元组 |
| `0x0D109050`/`60`/`70`/`80`（+ 类别 0–2） | Decal ID/Transform/Depth/Material | Key/Transform/Float/二进制 | 3 类别 × 基址偏移 |
| `0x0C12EF2X` | ecoUnitBinDrawBinIDs | Key 列 | prop 原型引用；**离线包内不可解析**（游戏内部编译表） |
| `0x0C12EF30` + bin | Prop Transforms | Transform 列 | 14 分箱（bin 0–13） |
| `0x0C12EF40` + bin | Prop Slots | 槽位值列 | 同上 |
| `0x0CAA680D` / `0x0CB00ED8` / `0x0CAA6832` | Path Points/Tangents/Indices | Float3/Float3/Int32 列 | 路径点 |
| `0x0CAA6841` | PathPairs | Int32 对 | 路径区间（语义待定） |
| `0x0E1BAC61` / `0x0E1BAC62` | Spawner IDs/Transforms | Key/Transform 列 | 生成器 |

### RW4（0x2F4E681B）

RenderWare4 容器：header、section index、Blob、Mesh/VertexFormat/Triangle/Texture/BBox。材质最多挂 6 个槽位——slot0 参数表（f32）、slot1 颜色控制图、slot2 法线、slot3 shader map、slot4 调色板、slot5 内景图。贴图为 DDS（DXT1/5）内嵌。目前只读解码，写回器为资产开发主线的第一优先事项。

### Raster 与 Terrain Map（0x2F4E681C / 0x03E421EC-ED-F0）

Raster 是无头裸像素（LotMask 为 raw RGBA，128²/256² 常见）。Terrain 系列共用 20 字节大端头 `[0, width, height, channel_code, byte_count]`：code 1 = 8 位灰度、2 = RGBA、**7 = 大端 u16 单通道**（16 位高度图，256² 实证）。

### 文本系（Locale JSON / JS / CSS / HTML / Shader / State Script）

Locale JSON 带 UTF-8 BOM（预览时剥离）。State Script（`0x024A0E52`）是游戏状态机脚本：`state Main -id 0`、`mode SimCity`、`cheat "..."` 等指令定义状态流转与启动参数。

### 音视频（Wwise / VP60）

Wwise 音频（`0x0D9E5710`）为 OggS/RIFF 变体，播放依赖 vgmstream；Bank（`0x0A4D8D09`）以 `BKHD` 开头。VP60（`0x376840D7`）为 VP6 编码过场视频。

### ER2 与 EP1 变体（0x08068AEB-AEE）

官方 ER2 Rule File（`AEB/AC`）为规则引擎二进制/文本。EP1 独有的 `AED` 是 gzip（`1F 8B`）包裹的 2.2MB 稀疏哈希表，`AEE` 是 12 字节记录表——类型号与 ER2 系列紧邻，判定为其 EP1 变体；结构未破解，暂以 hex 预览。

> 更深入的技术细节见仓库 `docs/overview/file-formats.md`。
