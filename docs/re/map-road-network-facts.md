# 地图道路网络：已确认链路与未解决问题

> 2026-10-12 更新：已定位 EP1 随包发布的原始 EcoGame bootstrap，并完成全部 15 图、130 城静态预览。当前来源、映射、地下水合成与复现命令见 [静态地图基线](map-static-bootstrap.md)。下文保留早期研究过程；“只能依赖存档”或“静态实例尚未定位”的结论已被该发现取代。

## 结论

真实道路的可视结果由以下链路共同决定：

```text
区域 EGB 的 UnitTransform + EcoPathSet 曲线
  → PathEntry (00B1B104:40E1C400:<id>)
  → static/realtime extrusion (00B1B104:09558B7E:<id>)
  → component/model
  → material/texture
  → GraphicsPath / GraphicsPathNetwork
```

道路不是 ED 中的一张道路贴图。ED 可以留下道路控制痕迹，但不能表达切线、交叉口连接、桥梁附件或交通网络拓扑。

## 静态道路 Unit

`SimCity_Game.package` 中的：

```text
00B1B104:40E1C000:30360939
```

已确认包含：

- `0x0CAA680D`：36 个局部路径点；
- `0x0CB00ED8`：36 个切线；
- `0x0CAA6841`：22 个索引，组成 11 条基础路径；
- `0x0CC8FE7A`：11 个 PathEntry；
- `0x00F9EFBB`：道路交叉口模型；
- `0x00F9EFBA`：包围盒。

Unit 本身是局部资源，不能直接当作 CapeTrinity 的世界道路。世界位置来自区域状态中的 UnitTransform，运行时还会根据连接器和交叉口切分基础路径。

## CapeTrinity 证据

干净新存档区域 EGB 有 66 个 Unit、118 条曲线。其中 `30360939` 出现两次：

```text
(981.13574, 2902.8171, -851.5662)
(-1626.1141, 1698.4719, -844.92)
```

118 条曲线只使用 11 种 PathEntry ID；扫描 `SimCityData` 后，唯一匹配这些 PathEntry 的静态 Unit 是 `30360939`。这证明区域曲线是同一个静态道路 Unit 的实例化和运行时切分结果，而不是另一张独立道路图片。

证据：`tmp/save3-region.json`、`tmp/static-path-match-save3.log`、`tmp/dump-30360939.log`。

## 28 行与 12 行外观表

道路外观表 `00B1B104:09558B7E:DC6C325F` 有 28 行；PathEntry `017A430A` 的静态挤出配置有 12 行。两者都是组件表，不能当作道路中心线坐标。

关键字段：

| 字段 | 语义 |
|---|---|
| `0x09558821` | MeshExtrusionMaterial 材质 Key |
| `0x09558827` | 每行模型/组件回退 Key |
| `0x09558828` | sweep 起始距离 |
| `0x09558829` | 重复间隔 |
| `0x0955882A` | sweep 长度；负值从末端锚定 |
| `0x0955882B` | 重复步长 |
| `0x0955882C` | 组件局部 XYZ 偏移 |
| `0x0955882D` | 是否沿曲线扭曲 |
| `0x0955882E` | 是否重复 UV |
| `0x0A49AEA4` | 纹理覆盖 Key |

`017A430A` 已确认同时引用：

```text
0x09532375 → static extrusion 24789770
0x09532377 → realtime extrusion 2363035178
```

OpenSCP 的 `crates/sc-properties/src/map_assets.rs` 已把 PathEntry 解析为 ribbon 和 model sweep，并保留模型、纹理和未解析组件清单。前端 `map-native-roads.ts` 按 EGB 曲线的 `entryId` 连接这些表。

## 已确认的 sweep 规则

- interval 为 0 时覆盖可用路径范围；
- length 为负数时从区间末端锚定；
- roundIntervals 会同步缩放区间、起点、长度和步长；
- instance 模型使用 `scale.x` 作为统一缩放；
- ribbon 的重复 V 使用 `uvStart.y - distance / worldSize.y`，不乘 `scale.y`；
- unresolved component 会回退到模型列，否则会丢失桥面、桥缆或附件；
- EGB 曲线可能存在分支和序列化浮点误差，不能要求整个 path 只有一条严格首尾相等的链。

## 当前已实现与验证

`curveChains()` 已按 PathEntry 分组，使用 0.25m 端点容差拆分连续链，避免一个分支导致整条 sweep 被丢弃。道路相关 Vitest 当前 7/7 通过，TypeScript 检查通过。用户已确认道路网络连接正常。

## 未解决问题

1. 桥梁两侧挡土墙和隧道附件仍有模型被展开成水平混凝土面的情况。道路中心线是连通的，因此这是模型轴、组件局部旋转或具体 RW4 资源选择问题，不是网络拓扑问题。
2. `0x09532377` realtime extrusion 已定位，但预览目前主要使用静态 extrusion；运行时动态道路外观仍需单独对照。
3. EGB 中的 118 条运行时切分曲线还不能只由 22 条静态基础段无损重建。
4. 没有 EGB 时，OpenSCP 还不能仅凭 package 生成游戏可接受的完整区域道路状态。

## 回滚锚点

当前道路预览基线提交：`dea5f677f22558f887c5138a3b2679073538077a`。
