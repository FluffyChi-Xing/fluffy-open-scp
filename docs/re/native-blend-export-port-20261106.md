# 原生 Rust 导出管线（进行中，2026-11-06）

## 目标

Asset Editor 插件不随运行时发布。Blender 只作为 `.blend` 读取器；
所有导出决策在 Rust 内完成。模块按职责拆分，纯函数、无副作用，
I/O 只在命令层。

## 数据边界

`blender_scripts/extract_blend.py`（我们自己的读取器，**不 import ny_facade**）：

- 声明与插件同名的 `NYLayer` 六字段 stub，让 Blender 绑定已存的
  `object.ny_layers`；
- 导出对象/矩阵/网格（顶点、面、loop、UV 层、FACE 属性）、材质自定义属性、
  节点树（节点/连线/socket 默认值）、场景单位、图片引用。

已用 `tall_office.blend` 验证：

```text
OPENSCP_EXTRACT_OK 3 objects / 276 materials / 1149 node trees
051: 12 ny_layers, 2450 verts, 2145 polys,
     8 UV layers, 17 FACE attributes
bundle.json ≈ 62.7 MiB
```

## Rust 分层

| 模块 | 职责 | 状态 |
| --- | --- | --- |
| `blend_native::dump` | 抽取 bundle 的 serde 模型 | ✅ |
| `blend_native::settings` | `settings_json` + 插件默认值/旧文档迁移 | ✅ |
| `blend_native::layout` | `layout_values` 窗格几何 | ✅ |
| `blend_native::appearance` | 逐窗亮灯/色温哈希 | ✅ |
| `blend_native::catalog` | pack/atlas/recipe 目录 | ✅ |
| `blend_native::resources` | 图集 PNG → 线性 RGBA，PNG 输出 | ✅ |
| `blend_native::channels` | 逐面状态（layer/material/recover） | ✅（recover 待做） |
| `blend_native::material_state` | 节点图状态恢复、UV 域/布局覆盖 | ✅（recover 待做） |
| `blend_native::recipe` | 每材质 tint/normal/shader/palette/interior | ✅ |
| `blend_native::geometry` | 三角化、裁切、切线、顶点流 | ✅ |
| `blend_native::decals` | 装饰贴花布局/裁切 | ✅ |
| `blend_native::glb` | GLB 组装（accessor/bufferView/image 去重） | ✅ |
| `blend_native::lotm` | LOTM v9 容器写出 | ✅ |
| `blend_native::export` | 编排；唯一组合模块 | ⏳ |

当前 23 个单元测试通过 + 1 个导出集成测试（默认值、旧 settings 迁移、layout 数学、
face_seed/choices、catalog recipe、sRGB/PNG 往返、layer/face_state 解析）。

`tall_office.blend` 的实际解析覆盖面：

```text
12 materials -> object ny_layers (WALL)
58 materials -> ny_face_state (WALL)
70 materials -> ny_face_state (WINDOW)
 0 materials -> node-graph recover
```

## 现有 Blender 导出不受影响

项目里已有的 `.blend` 导出是 `export_lot_model_blend`（LOTM → 可编辑
`.blend`），脚本为 `blender_scripts/export_lotm_blend.py` +
`tools/blender_asset_pipeline/b4_shader.py`。它消费的是 LOTM v9 容器和
六槽编号，本次导入改动没有修改容器格式或槽位顺序；导入改为只落 LOTM 后，
往返链路反而变成单一格式。回归测试：

```bash
SC_LOTM_FIXTURE="D:/ea-games/simcity_offline/entity/tall_test.lotm" \
cargo test -p fluffy-open-scp real_lotm_exports_editable_blend -- --ignored
```

结果：`blend bytes: 29530097`，17.6 s 通过。

`tools/blender_asset_pipeline` 是独立的六槽契约 GLB 导出（探针 mesh.json
输入），与本次导入路径不共享状态，不受影响。

## 验收基准与版本事实（2026-11-06 实测）

mods 里的两个 LOTM 不是当前插件版本产出的：

| 资产 | 参考 LOTM | 插件版本 | mesh/material | windows/decor |
| --- | --- | --- | --- | --- |
| 商业 | `商业地标资产开发/.../b04ca94a….lotm` | 1.18.1 | 105 / 105 | 1056 / 1116 |
| 大学 | `大学-综合实验楼-…/1b05299d….lotm` | 1.19.2 | 74 / 74 | 338 / 225 |

用 1.23.7 从 `entity/Commercial.blend`、`Institute.blend` 重新导出：

| 资产 | 1.23.7 输出 | mesh/material | windows/decor |
| --- | --- | --- | --- |
| 商业 | `tmp/accept/Commercial.lotm` | **109 / 109** | 1056 / 1116 |
| 大学 | `tmp/accept/Institute.lotm` | 74 / 74 | 338 / 225 |

结论：**同一 blend 用不同插件版本也无法逐字节复现 mod 里的 LOTM**（商业材质
分组从 105 变 109；大学的 tint/normal/shader PNG 字节不同）。因此最终验收分两层：

1. **逐字节/逐像素**：Rust 输出 vs 同一 blend 的 1.23.7 插件输出（主 oracle）；
2. **结构/语义 + 视觉**：Rust 输出 vs mods 里的旧版 LOTM（mesh/material 数、
   windows/decor 数、uvKind、槽位尺寸；差异必须能归因到插件版本）。

## 主验收基准

Rust 导出不得以“看起来差不多”验收。基准是插件自身的输出：

```text
tall_office.blend + SC5 Asset Editor 1.23.7
  -> 57,818,904 B
  SHA-256 185412dd5057ff9020e7609dd27e15189d59102e83f131800b67535f4d9fa8b8
```

PNG 压缩字节可能因 zlib 实现不同而不同，因此 Rust 侧按层比较：

1. 容器：mesh/material 数量、uvKind、diagnostic 计数；
2. 顶点流：POSITION/NORMAL/TANGENT/TEXCOORD_0..3 逐 float；
3. 每材质：params 4×float4、palette/tint/normal/shader/interior 解码后
   逐像素；
4. GLB 结构：primitive 分组、material 命名、image 去重。

## 切换策略

在 Rust 管线通过上述对拍前，`.blend` 导入仍使用 vendored 插件的原生导出
（golden 已逐字节验证）。Rust 导出通过验收后再删除插件路径；不会在管线
未完成时提前切换，避免再次出现“能导入但结果是错的”。

## 2026-11-06 复查：导入失败根因与逐层对齐

症状：`tall_office.blend`（以及 `Office.blend`）导入报
`原生导出失败：装饰贴花需要平面，请先把弯曲面拆成平面`，整包无法导入。

三条根因（都只在带旋转矩阵 / schema≥2 贴花的文件上暴露）：

1. **抽取器丢掉了全部 CORNER 域属性。** `extract_blend.py` 用
   `foreach_get("vector", [None]*N)`；Blender 要求扁平的 `N*dim` 浮点缓冲，
   实际抛 `RuntimeError: internal error setting the array`，又被
   `except (RuntimeError, TypeError)` 静默吞掉。于是 `NY_DecalCoord`
   （schema≥2 面保存的逐 loop 贴花坐标）从未进入 bundle，导出只能回落到
   `decals.frame()` 重投影；面一旦非平面就中止。修复：扁平缓冲 + 按维度
   拆分（`FLOAT_VECTOR`=3、`FLOAT2`=2）。
2. **法线矩阵少了一次转置。** `decals::normal_matrix` 与
   `export::world_normal` 实际返回的是 `M⁻¹`，而法线需要 `(M⁻¹)ᵀ`。物体矩阵
   含轴置换旋转时（tall_office `051`、Office 的所有对象），世界法线被转到
   错误轴：平面校验误判、写进 LOTM 的法线也错。修复后法线多重集与插件
   100% 一致（修复前 74%）。
3. **贴图颜色链路把原始字节当成了线性值。** 插件通过
   `bpy.types.Image.pixels` 读取 pack 图，对 8-bit 图返回的是**原始缓冲**
   （色彩管理只在着色时生效），所以它的 `srgb()` 是把原始值当线性再编码
   一次。我们此前 `decode_png(srgb=true)` 先把 palette/interior 解码成线性，
   导致这两张图整体偏暗（palette 第 14 行源字节 68，插件输出 141，我们 68）。
   修复：所有槽位保持原始值；字节→浮点用 `b * (1/255)`（与 Blender 位级
   一致）；编码取整改为 numpy 的 half-to-even。

同一轮修正并已逐像素对齐的细节：

- palette 非激活行仍要乘材质色；窗口分支把第 14、15 行的 red 清零；
  `np.roll` 的移位要从快照读源行（回绕时不能读已改写的行）。
- 浮雕 gain = `strength * min(|rect.w|*0.5, |rect.h|) / 0.16`，R 通道全量
  重映射并 clamp（此前直接用 strength、且只处理 R>FLAT 的像素）。
- 窗框色取**面状态**的 `frame_color`（会被 `NY_Tint` 覆盖），不是场景设置。
- 窗户预设按 256 像素矩形 `/256` 现算（`0.1640625`），不能预舍入为 `0.1641`。
- Office 内饰：`office_room_sheet` 重建 4 变体 × 4 状态（仅 `state==1` 保留
  alpha），512×512 输出。
- 默认墙/顶色取 `channels.defaults()` 的 0.8 灰，而不是 UI 属性默认值
  `(.62,.57,.47)` / `(.13,.18,.18)`。

对拍结果（Rust 原生 vs SC5 Asset Editor 1.23.7，同一 `.blend`）：

| 资产 | 插件 SHA-256 | 原生 SHA-256 | 分组 | 8 槽像素 |
| --- | --- | --- | --- | --- |
| Institute | `1dbbc4f3…` | `a7118dc1…` | 74 / 74 | 61/62 对逐像素一致；1 对 tint 差 1/255（81 px） |
| Commercial | `2e52a463…` | `6633f762…` | 111 vs 109 | 69/69 对逐像素一致 |
| Office | `9cee4ad8…` | `876d7c56…` | 66 / 66 | 40/40 对逐像素一致 |
| tall_office | `185412dd…` | `0fd7fd3a…` | 112 / 112 | 68/68 对逐像素一致 |

几何层：tall_office 顶点法线多重集 100% 一致；三角形总数一致，个别多边形
只差三角化对角线（单组面积差 ≤0.12 m²，视觉不可见）。Commercial 多 2 个
分组（两组状态只在 `night_variation` 的 f64 精度上不同，导致 key 分裂），
两组的 8 张图与插件完全一致，渲染结果相同。

5 轮确定性：`Institute a7118dc1…`、`tall_office 0fd7fd3a…` 各 5/5 一致。
导入缓存键升至 `openscp.blender.2026-11-06.4`，旧缓存（用旧抽取器生成）
一律失效重导。
