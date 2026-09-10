# SimCity 资源文件类型简介

SimCity (2013) 的所有游戏数据都打包在 `.package` 文件（DBPF 容器）中，每个资源由三元组 **TGI**（Type / Group / Instance）唯一标识。以下是模组制作中最常接触的文件类型。

## 常见类型一览

| 类型 ID | 名称 | 用途 |
|---|---|---|
| `0x00B1B104` | Property | 属性表：游戏变量、建筑参数、lot 定义 |
| `0x2F4E681B` | RW4 | 模型/纹理容器（RenderWare4）：Mesh、贴图、材质 |
| `0x2F4E681C` | Raster | 原始像素图：LotMask 地面掩码等 |
| `0x0A98EAF0` | Locale JSON | 本地化字符串表（UTF-8 BOM + JSON） |
| `0x0D9E5710` | Wwise 音频 | 音效/音乐（Wwise 格式） |
| `0x0A4D8D09` | Wwise Bank | 音频 bank |
| `0x376840D7` | VP60 视频 | 过场视频（VP6 编码） |
| `0x2F7D0004/06/07` | PNG/TGA/GIF | 标准位图 |
| `0x03E421EC/ED/F0` | Greyscale Map | 8/32/16 位灰度/彩度图（大地形） |
| `0x276CA4B9` | TrueType Font | 游戏字体 |
| `0x0469A3F7` | Shader 源 | HLSL 着色器片段 |
| `0x02393756` | Cursor | 光标（ICO/CUR 容器） |

## 关键概念

- **TGI 寻址**：资源的引用（如 lot 指向模型）都以 Key 属性存储，跨 package 生效。
- **Property 表**：数据驱动的核心——引擎只解释属性，行为语义全部在数据侧。
- **Lot 四件套**：`LotSize`（地块尺寸）、`LotMask`（四通道地面掩码）、
  `LotPlacementTransform`（模型↔地块定位）、`LotColor1-4`（通道颜色 +
  Alpha=地面贴图索引）。
- **材质槽位（Material Set）**：RW4 材质最多挂 6 个槽位——slot0 参数表、
  slot1 颜色控制图、slot2 法线、slot3 shader map、slot4 调色板、slot5 内景图。

> 更深入的技术细节见仓库 `docs/overview/file-formats.md`。
