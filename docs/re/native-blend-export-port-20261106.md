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
