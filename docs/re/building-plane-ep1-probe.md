# EP1 四类建筑平面合成（2026-10-21）

## 结论与边界

四个给定 Property 均成功提取并合成。砖墙、石墙、工厂格窗、玻璃幕墙的图集选区与双层混合可以用于程序化白模表面；光照、玻璃反射、内景亮度尚不能判为游戏等价。

本次发现旧探针只取首个 binding 不适用于这些资产。四个模型均有两个 mesh→material binding：binding 0 使用同一套 EP1 未来化图集，binding 1 使用各楼自身的墙面图集。两套共享同一张参数表，但**使用的列不同**，不能把全部参数列都套在任意一套图集上。

| Property instance | LOD1 instance | 参数表列数 | binding 0 / 1 实际使用列数 | 主体样片 |
|---|---|---:|---:|---|
| 35dd7d33 | 35dd7d33 | 57 | 11 / 46 | 红砖、浅色壁柱、带窗眉窗台的窗 |
| 1e89c4e3 | 1e89c4f8 | 227 | 75 / 43 | 砖墙、密格窗、菱形饰件 |
| d1f40247 | d1f40248 | 54 | 31 / 22 | 石墙、窄窗、大窗、装饰带 |
| 4cc44d42 | 4cc44d4d | 69 | 21 / 48 | 竖向玻璃、框架、蓝色窗间板 |

未来化图集五个共享槽位：slot1=1188B12E、slot2=A3791E5C、slot3=110BF822、slot4=19525AE3、slot5=206064AE。主体五槽位分别记录在输出 results.json，均读取真实 package。原始截图只作视觉参考，未从截图裁切任何像素用于合成。

## 重跑

依赖 Rust、Python、numpy、Pillow。在项目根目录使用 PowerShell：

```powershell
$data = 'D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData'
cargo build -p sc-exporter --release --example building_material_probe
$tgis = '42e1c000:35dd7d33','43e1c000:1e89c4e3','42e1c000:d1f40247','42e1c000:4cc44d42'
foreach ($tgi in $tgis) {
  $instance = $tgi.Split(':')[1]
  foreach ($binding in 0,1) {
    $out = "tmp/ep1_planes/$instance"
    if ($binding -eq 1) { $out += '/binding1' }
    & target/release/examples/building_material_probe.exe "$data/SimCityDataEP1.package" `
      "--property=00b1b104:$tgi" "--binding=$binding" "--out=$out" `
      "$data/SimCity_Graphics.package" "$data/SimCity_App.package"
  }
}
python tools/building_ep1_probe.py tmp/ep1_planes
```

输出目录 `tmp/ep1_planes`：

- `comparison.png`：四张 768×768 方形合成图的总览。
- 每栋的 `square_display.png`：含简化内景与固定天空代理的观察图。
- `square_albedo/overlay/opacity/ao/height.png`：合成的材质中间量。
- `square_base_only.png`、`square_top_difference.png`：关掉 Top 的单变量消融。
- 两个 binding 的 `used_columns_motif.png`：只列出该网格真正使用的参数列；motif 模式反解 padding，故用于看完整图案，不代表原始平铺密度。
- `params.json`、`mesh.json`、`slot*.png`：原始参数、顶点/双 UV、解码贴图。
- `results.json`：各 binding 的槽位 ID、使用列、网格数量、合成配方、通道范围、窗洞比例和 Top 消融差异。

## 判读

主体布局是明确编写的方形探针配方，不是原建筑截图复原。公寓、工厂、石墙样片能把墙体与窗口分开控制；幕墙列 3 的 padding 为约 (-0.05,-0.03)，Top 可以满铺，和墙面局部开窗不同。忽略负 padding 或强行使用普通墙的窗口留白，会错误地产生幕墙缝隙。

未来化/广告牌 binding 的真实使用列已分别导出，但其运行时广告内容、动画和发光不在本次静态 building4 合成的验证范围；不声称已还原广告牌在游戏中的完整效果。

验证：8 个 binding 均可读，使用列全部在参数表范围内；四张主体图及通道全部为有限数值；Top 消融确实改变外观，幕墙变化覆盖全域。已目视检查主体总览、三类逐列图及工厂前 48 列。颜色变体固定为 0，未拟合截图光照；mip-0 双线性采样、简化内景、没有视差步进/真实 cubemap，故图像锐度、玻璃与灯光仍有预期差异。
