# 地图资源图层：brush、ED 与 typed map

> 2026-10-12 更新：已定位 EP1 随包发布的原始 EcoGame bootstrap，并完成全部 15 图、130 城静态预览。当前来源、映射、地下水合成与复现命令见 [静态地图基线](map-static-bootstrap.md)。下文保留早期研究过程；“只能依赖存档”或“静态实例尚未定位”的结论已被该发现取代。

## 结论

资源图层有三种不同层次，不能混用：

```text
静态区域属性 / brush 参数
  → 新区域初始化时的程序化 stamp
  → EcoMap typed map
  → 游戏资源视图与模拟系统
```

ED 是地表生态/控制场的一部分，不是所有资源的最终分布图。城市 EGB 中的资源 map 是城市模拟范围内的状态快照，也不能代替 pristine 区域资源。

## 静态资源 brush

Cape Trinity 的静态区域属性已经读出以下 brush 目标：

```text
coalEcoMapBrushes
oreEcoMapBrushes
oilEcoMapBrushes
waterTableEcoMapBrushes
forestEcoMapBrushes
```

相关字段链：

| 字段 | 当前解释 |
|---|---|
| `0x00B2CCCA` | brush 目标列表 |
| `0x02A907B5` | brush Key 列表 |
| `0x02A907B6` | 世界坐标变换/强度 |
| `0x0DBA3A9C` | 目标 map 名 |
| `0x0DE43899` | typed-map 索引 |

这些字段能证明资源来源与位置参数在静态区域数据中存在，不依赖联网或玩家城市存档。

## 新区域初始化时的 EcoMap

反编译链显示，`cEcoMap::Create` 会：

1. 分配 map cell 数组；
2. 从 map-entry property 的 `0x7E0AE06` 查找图像资源；
3. 优先尝试 16-bit image type `65282544`，再尝试 8-bit image type `65282540`；
4. 按 `mMaxValue` 缩放样本；
5. 视需要应用 `0x7E0AE02` 随机 splat、半径 `0x7E0AE03`、值 `0x7E0AE04` 和可选 wrap 字段。

因此高度、土壤、森林、地下水及部分资源基础层在新 EGB 之前就可以在内存中生成。

## ED 的范围

当前证据支持：

- ED 参与地表材质、生态密度和道路痕迹等视觉控制；
- ED 的森林通道不能直接当成树实例清单；
- ED 的道路通道不能恢复车道线、PathEntry 或网络连接；
- ED 可以作为无存档预览的地表控制层，但不能伪造真实道路。

## 预览现状

OpenSCP 当前已经支持：

- 土壤、森林、地下水等生态层的预览；
- 资源图层选择、透明度与显示/隐藏；
- brush 目标的可见性分组；
- 不依赖城市存档的静态 brush 参考。

当前 brush 主要作为位置/范围覆盖层使用，尚未完全复刻游戏的 typed-map 合成、强度叠加、等值线色带和资源视图 shader。因此“能看到 brush”不等于“已经还原游戏资源分布”。

## 未解决问题

1. brush stamp 的完整半径、强度、随机种子、叠加顺序尚未逐字段确认。
2. 资源 typed map 的初始图像、程序化 brush 和城市模拟覆盖之间的优先级需要更多空存档对拍。
3. 未对齐存档时，OpenSCP 仍缺少一个完全等价于游戏初始化的资源合成器。
4. 资源图层与基础地表材质的深度、裁切、缩放和夜间显示还需要更多地图样本验证。

## 关键边界

后续地图编辑应把资源修改拆成两类产物：

- 静态 package 中的区域 brush/map-entry 覆盖；
- 区域或城市 EGB 中的 typed map 状态覆盖。

只修改其中一层，不能保证游戏内资源视图和模拟结果同时变化。
