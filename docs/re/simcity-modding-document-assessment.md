# `simcity_modding.pdf` 对 OpenSCP 的第二轮证据评估

日期：2026-10-17

这份文档共 96 个 PDF 页，内容是 SimCity 建筑/对象制作与导入教程集合。它描述的是已经被社区实际使用过的制作流程，但其中的 3ds Max、Photoshop、OpenCOLLADA 等外部工具步骤不能直接视为 OpenSCP 已实现的能力。

## 这篇文档补充确认了什么

### 建筑资源不是单一模型

建筑教程（PDF 页 191–218、228–256）反复使用以下资源链：

```text
建筑 PROP/Property
  ├─ 多个 LOD 的 RW4 模型
  ├─ 材质与纹理/调色板
  ├─ Lot Property
  ├─ lights / spawners / decals / props / transforms
  └─ 菜单图标、Marquee 与文字引用
```

教程先按 Instance 搜索建筑，再导出最大模型 Property，沿着 LOD 字段取得多个 RW4；随后把模型导入 3ds Max/Blender，制作材质和纹理，最后在 SimCityPak 中导入模型、纹理和 Property，并保存为新的 package。

这与 OpenSCP 已有的资产研究完全一致：社区 package 不能简化成“一个模型加六张图片”。

### LOD 是 Property 中的 Key 依赖

PDF 页 193–194 说明：

- 同一建筑的 LOD 使用相同的主体 Instance ID，仅尾部字母/数字不同。
- 先从 Property 读取 LOD 字段，再定位对应 RW4。
- RW4 资源需要保持原始 Instance 命名/身份，不能任意改名。

这与原始 C# 的 `UnitLOD1`–`UnitLOD4` 常量和菜单复制时递归复制 LOD 的实现相互印证。OpenSCP 已能解析和预览这些依赖，但仍没有通用 RW4 写回器。

### Lot Editor 是可操作的数据编辑器

PDF 页 219–220、258–268 展示了 Lot Editor 的实际编辑面：

- 导入/替换 Lot Raster 地面纹理；
- 修改 Lot Size、Lot Mask、四种地面材质颜色和边界颜色；
- 编辑 lights 的颜色、半径、强度等参数；
- 查看和调整 spawners、decals、props、transforms；
- 保存 package 后重新打开，并进入游戏检查建筑近景和灯光。

这不是单纯的预览功能，而是一个已被验证的“Property 数组/字段编辑 → package 写回 → 游戏检查”工作流。

## 与当前 OpenSCP 的对照

| 文档操作 | OpenSCP 当前状态 | 判断 |
| --- | --- | --- |
| 搜索建筑 PROP/Property | 已有资源索引、Property 预览和语义卡 | 已关联 |
| 解析 LOD、RW4、材质依赖 | 已有模型/材质预览与依赖研究 | 读取可用，写回缺失 |
| Property Parent 展平 | 已有解析和语义层 | 已关联 |
| Lot Size、Mask、颜色、灯光字段 | Property Editor 已有字段模型和可视化 | 本地编辑可用，真实 package 写回仍不完整 |
| props、spawners、decals、paths | `sc-properties` 与 Property Editor 已有解析/预览 | 数组级写回和依赖分配未闭环 |
| 导入 Lot Raster | 有 raster 编辑和 overlay 相关能力 | 需与 Lot Property 的资源引用统一验收 |
| 修改 RW4/材质并重新导入 | 有 RW4/材质预览 | 没有稳定的 RW4 编译/写回路径 |
| 保存 package 并重新打开 | DBPF overlay writer 和重读测试存在 | 尚缺固定样本的游戏内验收 |

## 能直接转成真实验证的最小案例

这篇文档把下一步从“菜单”扩展成三个可控案例。它们都可以复用现有 Property parser 和 `patch_property_overlay`，不需要先实现任意模型编译器。

### 案例 A：Lot Property 的标量字段

选择一个已经能预览的建筑 Lot：只修改 Lot Size、四种颜色或单个 light 的标量字段。

验收顺序：

1. 读取源 Property，记录字段 hash、类型和原始值；
2. 写出单资源 overlay；
3. 重新打开 overlay，逐字段比较；
4. 在游戏中检查地块尺寸/颜色/灯光变化。

这类修改最接近当前 `patch_property_overlay` 的能力边界。

### 案例 B：Lot Raster 替换

使用文档页 219 的彩色 Raster/Decal 示例：保持 Property 和 TGI 不变，只替换已知 Raster 资源，验证 Lot Editor 和游戏中的地面颜色分区。

该案例可以复用现有 raster writer，但必须同时验证：

- Raster 的尺寸、通道和 mip；
- Lot Property 对 Raster 的 Key 引用；
- OpenSCP 预览与游戏中颜色分区是否一致。

### 案例 C：单个 Prop/Light 数组成员

选择一个已有 Lot，修改一个 prop transform 或 light color/radius。它能验证数组成员编辑、序号稳定性和 Property 写回，而不需要添加新的资源依赖。

## 不能从这篇文档直接承诺的能力

- 3ds Max MAXScript、OpenCOLLADA 导入和任意建筑生成；
- 任意 GLB/OBJ → 游戏 RW4 的编译；
- 任意材质节点 → SimCity 专用 shader/raster 的自动转换；
- 自动推断 Lot 的功能语义、道路接口、碰撞和模拟参数；
- 仅凭 OpenSCP 预览就宣称游戏内视觉一致。

文档中的流程依赖人工选择资源、人工制作纹理、人工检查游戏结果；OpenSCP 可以逐步替代其中的“查找、预览、字段修改、打包和对拍”，但不能把外部 DCC 工具链自动化当作已经完成。

## 对当前路线的调整

第一篇文档支持先做菜单 Parent overlay；第二篇文档说明菜单之后应优先补齐一个 **Property-only Lot 写回案例**。推荐顺序为：

1. 菜单 Parent Menu 移动（最小 M1）；
2. Lot Size/颜色/light 标量写回（Property-only M2）；
3. 单个 prop/light 数组成员写回（受白名单约束的 M3）；
4. Lot Raster 替换与引用完整性验证（M4）；
5. 最后再做 Menu2 复制、依赖资源复制和 RW4 写回。

这样每一步都有文档中的真实操作作为基准，并且能把“OpenSCP 预览正确”逐步提升为“生成 package 可重读、可安装、游戏中可观察”。

## 证据位置

- `C:\Users\FluffyCHi_Xing\Desktop\simcity_modding.pdf`，PDF 页 191–220：建筑资源查找、LOD/RW4 导出、材质/纹理、Lot Editor。
- 同文件 PDF 页 228–256：现代消防站完整建筑制作和 Property/LOD/材质导出。
- 同文件 PDF 页 258–268：Raster、Lot Editor、lights、spawners、decals、props 和游戏内检查。
- 原始实现：`D:\source-map\Simcitypak-v2\SimCityPak\Views\AdvancedEditors\LotEditor\PropertyConstants.cs`。
- OpenSCP 当前边界：[asset-flow-integration.md](asset-flow-integration.md)。
- OpenSCP 菜单证据：[scp-verified-operation-integration-assessment.md](scp-verified-operation-integration-assessment.md)。

