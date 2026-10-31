# Blender 闭环迁移到 Rust 后端

Blender 插件的可用闭环分成三层：

1. **LOTM v9**：几何按编辑器材质组写入容器；它是交换格式，不是可直接进入游戏的 RW4。
2. **原生 RW4 骨架**：从已知可用的 NightHouse/建筑 RW4 读取 section 顺序、对齐间隙、类型表、CollisionMesh、BBOX 和 Material；只替换索引、顶点、VA/TA/Mesh 等几何 payload。
3. **Lot Property**：按插件 `lot_native.py` 的 20 个字段写入 Lot Property，并用独立 mask Raster 和 surface RW4 TGI 形成资源闭包。

Rust 侧已落地：

- `asset_build_m1::native_lot`：插件 Property 字段和大端编码的 Rust 实现；
- `rw4::writer::replace_template_sections`：保留原生 RW4 容器骨架，仅替换指定 section payload；
- LOTM 超过 4 个 mesh 自动进入安全形状路径；
- 写包前拒绝超过 4 mesh 的输出；
- native model 使用原生 CollisionMesh 模板。

剩余接入工作是把 `native_lot::NativeLotSpec` 接到 M1 schema 的 lot/mask 输入，并让模型模板替换使用真实 reference RW4；这一步完成后，构建器才会同时拥有插件验证过的模型、材质和 Lot Property 闭环。
