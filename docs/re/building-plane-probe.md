# 建筑六槽位平面合成探针

`building_material_probe` 现在接受 property TGI：

```text
cargo run -p sc-exporter --release --example building_material_probe -- \
  D:/.../SimCity_Graphics.package \
  --property=00b1b104:40a2a10b:10daf7bf --out=tmp/building_probe_10daf7bf
```

它从 property 的 LOD1 解析 RW4 模型，沿 mesh → material 绑定找到六个有效槽位，导出槽位 PNG、完整参数表 `params.json` 和原始顶点/双 UV/材质列 `mesh.json`。随后运行：

```text
python tools/building_plane_probe.py tmp/building_probe_10daf7bf
```

输出 `columns_base.png`、`columns_padding.png`、`columns_motif.png` 和 `plane_06/07/27_*`。这组图是可复现的 mip-0 CPU 探针，专门把材质列的差异和 Top 层窗口区域显式展开。

## 当前证据

给定 property `00b1b104:40a2a10b:10daf7bf` 的 LOD1 是 `2f4e681b:00000000:c551dc34`。其材质为：

| 槽位 | 内容 | 作用 |
|---:|---|---|
| 0 | 38×4 float 参数表 | 每个材质列的 palette、Base/Top region、padding、内景缩放 |
| 1 | 256×256 RGBA | facade tint：调色板坐标、亮度、Top 覆盖率 |
| 2 | 256×256 RGBA | normal/AO，alpha 是 AO |
| 3 | 256×256 RGBA | shader map，alpha 是窗洞覆盖，x/y 是 relief 数据 |
| 4 | 512×16 RGBA | palette，RGB 是颜色，alpha 是表面参数 |
| 5 | 512×512 RGBA | interior 图集 |

因此“六张贴图在正方形平面上合成”的关键不是六等分贴图，而是：顶点的材质列选择参数表列，Base UV 选择墙面区域，Top UV 选择窗口 motif 区域，tint 的 alpha 决定两层混合，shader alpha 决定外观/内景混合。

该资产的窗口列 6/7/27 复现出重复砖墙上的窗口排列。更正：本资产列 7 的 Top `regionXform2.x` 为正值 0.1796875；先前将其它资产的负缩放结论误写到了此列。采样器支持负 scale，但本样本不能单独作为镜像行为的实证。`padding` 会把 Top motif 约束到立面中段；把它简单等同于关闭会丢掉窗户。

探针的 `display` 图含固定天空代理，只用于观察组合关系；`albedo`、`overlay`、`opacity`、`height` 是后续三维白模映射更可靠的中间结果，不声称替代游戏的完整视差、光照和 cubemap。
