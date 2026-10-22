# EP1 金属住宅与全玻璃写字楼探针结论

运行：

```text
python tools/building_future_probe.py tmp/future_planes
```

输出：[comparison.png](../../tmp/future_planes/comparison.png)、[results.json](../../tmp/future_planes/results.json)，原始六槽位和参数/顶点记录分别在两个资产目录中。

## 资产定位

| Property | LOD1 | 材质绑定 | 参数表 | 观测 |
|---|---|---:|---:|---|
| `00b1b104:41e1c000:5a6ec67a` | `2f4e681b:00000000:5a6ec675` | 1 | 63×4 | 金属/未来住宅，窗、门、栏杆和锈蚀下沿共用未来化图集 |
| `00b1b104:61f0c000:1d261fdd` | `2f4e681b:00000000:30857cee` | 1 | 31×4 | 全玻璃幕墙，主体玻璃由 Base 区域铺贴，所选列 7 的 Top 不改变合成结果 |

两栋楼都在 `SimCityDataEP1.package`，都使用共享槽位：slot1 `1188B12E`、slot2 `A3791E5C`、slot3 `110BF822`、slot4 `19525AE3`、slot5 `206064AE`。差异来自参数表列和网格双 UV，而不是另一套纹理包。

## 结论

金属住宅的材质链可以正常生成深色金属板、内景中的青色亮条（简化内景结果，不是已验证反射）、窗洞和底部锈蚀面。它的 row3 padding 同时出现 `-0.95` 和极大值，说明必须保留负/大 padding 的语义；把任何非零 padding 都当作正常窗口区域会把金属附件错误铺到墙面。样片中窗口覆盖约 10.7%，Top 消融影响约 14.2%。

全玻璃写字楼的主体是 Base 层连续玻璃窗格：受控样片的窗洞/玻璃比例约 83.6%，Top 消融为 0。所选列 7 的这个实验说明“玻璃幕墙 = Top 窗口层”并不成立；不能为了复现幕墙而强行加入 Top motif。窗格与颜色变化来自 Base tint/shader/palette 组合以及其 UV 密度。

探针当前的方形布局是实验配方，不是原始三角网格的空间重建。`square_albedo`、`square_shader`、`square_surface` 和 `square_reflectance` 可用于后续三维白模；`square_display` 使用简化内景/固定天空代理，不能作为游戏光照或玻璃 cubemap 的像素级对拍。

该结论补充了前四栋 EP1 资产：建筑表面应按 mesh binding → material → 参数列 → 双 UV 选择材质，而不能只按“建筑风格”推断墙面/Top 层规则。
