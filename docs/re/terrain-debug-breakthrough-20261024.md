# 地图 debug 工具突破与游戏侧验证边界

日期：2026-10-24。

本文记录对 Reddit 所说“游戏运行时向地图添加地下资源”的复核结果。目标是把菜单、区域 brush、Terrain Editor、typed height map 和 341 资源写回链路逐段对拍，避免把残留的 Property 目录误判成可执行工具。

## 已确认的执行链

开发样本和零售反编译都保留以下调用关系：

```text
cTerrainRegion::Init
  -> cTerrainBrushes::Init
  -> cTerrainBrushes::Update
  -> RunScriptBrushes
  -> runTerrainScript visual effect
  -> cTerrainBrushEffect
  -> cTerrainEditor::ApplyBrush / ApplyGlobalBrush / ApplyRibbonBrush
  -> EndAccumulatingBrushes
  -> Save341HeightMaps
```

`RunScriptBrushes` 从 brush property list 读取：

| Property | 用途 |
| --- | --- |
| `0x02A907B5` | 主脚本 effect ID 数组（开发样本常量 `44631989`） |
| `0x02A907B6` | effect transform 数组 |
| `0x03A23F97` | effect 时间数组 |
| `0x03DD8E59` | effect seed 数组 |
| `0x03DD8E5A` | effect intensity 数组 |
| `0x0DBA3A9C` | height-map basename |
| `0x0DC097E3` | final resolution |
| `0x0DE43899` | typed height-map type |

其中后三项直接决定输出的是 terrain、water table、oil、ore 等哪一张 typed map。`Save341HeightMaps` 再将层合成为 mip 与 8192 tile，并用区域 341 key 写回资源。

## 零售包的关键缺口

隐藏 Property `00B1B104:09878A01:454041B4` 是 Terrain Brush 选择器目录，包含 222 个名字，覆盖：

```text
brush_add
brush_add_global
feature_hills_*
erosion_*
brush_region_<region>_<terrain|water|forest|oil|ore|coal>
box_deposit1xV1 / box_deposit2xV1 / box_deposit3xV1
```

这些条目的 `0x0D79C8ED` Key 全部是 `type=0, group=0` 的裸 instance。对安装目录全部 package 逐条扫描后，222 个 instance 的命中数都是 0。它们是目录中的名字和 ID，占位信息，不是可以直接被 Property Manager 读取的资源。

区域描述中同样保留了 `waterTableEcoMapBrushes` 等字段。`cTerrainRegion::Init` 的真实逻辑不是直接把这些字段当作 brush 描述，而是：

1. 从区域 property 的 `0x02A907B5` 取 brush instance；
2. 以“该 instance + 当前 region group”向 Property Manager 请求另一个 property list；
3. `cTerrainBrushes::Update` 在 property list 首次变更时调用 `RunScriptBrushes`。

对 `SimCity_RegionTerrain0.package` 和 `SimCity_RegionTerrain1.package` 的全部区域字段执行相同闭包审计，`waterTableEcoMapBrushes`、`oilEcoMapBrushes`、`oreEcoMapBrushes`、`coalEcoMapBrushes` 等引用都没有对应的 brush definition。因而当前零售包有资源 stamp 的坐标，却没有可供运行时刷写的 brush 脚本定义。

本次安装目录实测输出为 `keys=222 names=222`；区域闭包共检查 123 行，`definition_count=0` 的行占全部 123 行。

菜单 action 也不能绕过这个缺口：`454041B4` 的 action 是 `0x2A9AA359`，而零售 `SC::CreateTool` factory 只处理 `cToolPlop`、`cToolPathSelector`、`cToolRegion`、`cToolTornado` 等固定 ID，没有这个 case。`CreateTools` 对该 action 会得到空 tool，因此把菜单 Parent 或 slot 改成可见分类不会产生可执行地形工具。

## 这对“游戏侧验证”的含义

现在可以安全验证的是“残留目录和执行侧代码仍在”，不能安全宣称“覆盖包已经能添加地下水”。如果只覆盖 `454041B4`：

- 菜单可能因为没有 native factory 而完全不出现；
- 即使出现，也缺少 brush property list 和 child effect 资源，点击后无法走 `RunScriptBrushes`；
- 直接伪造一个空 property list 可能触发 null effect 或产生没有输出的假阳性，因此不生成这种包。

实际验证入口应满足以下闭包条件后再做：

```text
brush selector/menu
  + brush property list（region group）
  + cTerrainBrushEffect 的 child effect description
  + brush texture/kernel 资源
  + typed map basename/resolution/type
  + Save341HeightMaps 输出资源
```

最小实验仍应只改一个区域的一小块 water table：记录进入城市前的 typed map，触发一次 brush，保存并退出，重启后读取对应 341 tile/mip，确认数值变化且没有覆盖其他资源层。未完成上述 round-trip 前，不把地图 brush 接入 Mod Studio 菜单。

## 本轮工具与证据

- `probes/terrain/terrain_brush_resource_audit.rs`：读取 `454041B4` 的刷子目录，扫描所有安装 package 的 instance 命中，并对区域 brush 的“instance + group”资源闭包做审计。
- `tmp/dynamic/source-tree-impostor/src/SC/RunScriptBrushes.c`：脚本 brush 到 Terrain Editor 的调用证据。
- `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c`：`cTerrainRegion::Init`、`cTerrainBrushes::Update`、`CreateTool`、`Save341HeightMaps` 的反编译证据。

## 下一步

1. 从开发样本或遗留工具包寻找 `cTerrainBrushEffect`/`cTerrainScriptEffect` 的二进制 description，而不是继续猜测 Property 字段。
2. 为一个 water table brush 构造完整的 effect ID、transform、seed、intensity 和 typed-map property list。
3. 先在离线探针中验证 `PropertyFile` 与 Swarm description 的字节布局，再生成单区域、单资源层覆盖包。
4. 只有游戏重启后 341 typed map 仍保留变化，才把该入口标记为可用地图 debug 工具。
