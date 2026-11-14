# LOTM 资产打包管线：后端补全与公园对拍（2026-11-01）

## 0. 边界

本轮只完善后端 `asset_build_m1` 打包管线，**不接入前端 asset 工作台**。  
前端只规划能力，不新增页面、不改现有工作台按钮。

设计约束：

- 高内聚：一个模块只回答一个问题；
- 低耦合：格式发现、闭包解析、DBPF 组装、UI 编排分开；
- 函数式优先：纯函数只接收 bytes/Resources，返回 bytes/结果，不做文件 I/O；
- I/O 只在边界：`package.rs` 负责编排与 staging，`connector.rs` 负责 DBPF，`commands.rs`/CLI 只做入口。

## 1. 已实现（本轮）

### 1.1 `resource_closure.rs`（新增，纯函数）

职责：从最终 delta 的 Property/RW4 中发现外部引用，按 SimCity 的 TGI 约定解析，把参考包中的 bytes 提升进 delta，缺失必需引用则失败。

- LotMask `0x0CCB7FD5` 支持 `type/group=0` 的残缺 Key：先按精确 TGI，再按 `2F4E681C:group:id`，最后按 `2F4E681C:0:id`。
- 模型 MATERIAL 的 slot0 只接受 RW4 palette；slot1..5 接受 raster 或 RW4 wrapper。
- 结构性 Property Key（parent/unit/lot）不复制；图形 Key（RW4/raster/icon/marquee）若参考包有则可选复制。
- `complete()` 只写 `written`，不写 `resources` 全量；`required` 缺失直接报错。

### 1.2 `template_material.rs`（新增，纯函数）

职责：只改原生 RW4 模板 MATERIAL 的 6 个 `texture_instance`，保留模板 header、shader、附加数据、section 布局。

调用点：`native_model::compile_lotm_nh_template()` 在 `replace_template_sections()` 之后调用。  
结果：模型不再引用 Dalian/NightHouse 的旧纹理，而是本次 LOTM 生成的 6 个材质槽。

### 1.3 palette 编码修正

`materials::encode_params_rw4()`：

- `file_type` 从 `1`（Model）改为 `0x04000000`（Texture）；
- section 顺序从 `Texture → Blob` 改为 `Blob → Texture`；
- texture 的 data section 指向 Blob #0。

与 NightHouse palette 的 `file_type=0x04000000` 对齐。

### 1.4 delta 输出闭包 + 四个独立 LOD

- `build_from_schema()` 在 `written = delta()` 之后调用 `resource_closure::discover/complete`，再 link。
- `insert_compiled_model()` 为同一份 RW4 字节分配 4 个不同 instance。
- `retarget_lod_models()` 把 `F9EFBB..F9EFBE` 分别指向 4 个模型，而不是同一个 instance。

### 1.5 新身份 LotMask 重新分配

`menu::clone_lot_mask()`：

- 从参考 lot 解析 `0CCB7FD5`；
- 在参考包中按 SCP 语义找到 mask bytes；
- 以 `sha256("openscp/menu-lot-mask" | menu_instance | mask_bytes)` 分配**新的 raster instance**；
- 把新 mask 作为 `MenuResources.entries` 写入 delta；
- 把新 lot leaf 的 `0CCB7FD5` 改成新 TGI。

这样不会覆盖参考建筑的 mask，也不会和官方 DLC 的同 instance 冲突。

### 1.6 手动后端入口

新增 `src-tauri/examples/build_asset.rs`：

```powershell
cargo run -p fluffy-open-scp --example build_asset -- <project-root> <schema.json>
```

不经过 Tauri UI，不依赖 asset 工作台。

## 2. 公园对拍包（已安装）

目录：

`D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData`

| 资产 | 文件 | 大小 | SHA-256 |
|---|---|---:|---|
| 商业 | `000_OpenSCP_CommercialPark.package` | 52,631,307 | `E63B85B5…CC748F` |
| 大学 | `000_OpenSCP_UniversityPark.package` | 53,850,848 | `379F5E6C…B99C` |

菜单身份（均为公园模板，模仿 NightHouse）：

- parent `F970A822`
- unit template `565B72B9`
- lot template `34CE7E14`
- category `07049250`

| 资产 | menu instance | 新 mask instance | LOD instances |
|---|---|---|---|
| 商业 | `0x554FEC0B` | `0xB03A7AD0` | `0x3855A2C8..CB` |
| 大学 | `0x792BCB6D` | `0x1D29D152` | `0x6EC45766..69` |

离线验收：

- LotMask：两包均 `RESULT: OK`；
- 模型材质：两包均 `0 missing`；
- palette：`file_type=0x04000000`；
- LOD1–4：四个不同 instance；
- 与官方/DLC/Dalian 包无 TGI 冲突（本轮只复用同一 mask bytes 的情况已通过重新分配消除）。

进入游戏后，在“公园”菜单下应看到：

- `OpenSCP Commercial Park`
- `OpenSCP University Park`

## 3. 后续后端能力（按优先级）

### P0：闭包图从“单层”升级为“可递归图”

当前 `resource_closure` 只复制一层。下一版应：

- 用 `DependencyGraph` 表示节点（TGI）与边（Property Key / MATERIAL slot）；
- 复制进来的 RW4/Raster 如果自身还有引用，继续展开；
- 对循环引用做 visited set；
- 对“基础包可解析”的引用显式标注 `BaseProvided`，而不是静默跳过。

建议模块：`resource_graph.rs`，纯数据 + 纯遍历；`resource_closure.rs` 只负责把图结果落到 `written`。

### P0：`LodModelSet` 值对象

把 `[ResourceId;4]` 从 `package.rs` 的临时类型提升为 `model_set.rs` 的 `LodModelSet`：

- `LodModelSet::from_compiled(bytes) -> Result<Self>`
- `instances() -> [ResourceId;4]`
- `apply_to_property(PropertyFile) -> Result<()>`

这样 `package.rs` 不再知道“四个 LOD”的细节，`menu.rs` 也不需要知道数组下标。

### P1：菜单 profile 外置

把“公园/NightHouse”模板对从 `menu.rs` 常量提升为：

```text
menu_profiles/
  park.json
  landmark.json
```

`MenuProfile` 只描述 parent/unit template/lot template/category/default position。  
`menu.rs` 只负责生成 Property；profile 由 schema 选择。这样新增菜单类型不需要改生成器。

### P1：Property 克隆策略外置

`SAFE_LOT_HASHES` 目前是硬编码白名单。下一版改成：

- `LotClonePolicy`：保留字段集合 + 必须重新分配字段 + 禁止字段；
- 每个字段带证据/原因；
- 构建报告输出“保留/丢弃/重分配”三类明细。

模块：`lot_clone_policy.rs`，纯函数，输入参考 Property + policy，输出新 Property。

### P1：验证报告对象化

当前错误是字符串。下一版：

```rust
pub struct ClosureReport {
    pub dependencies: Vec<DependencyStatus>,
    pub missing: Vec<Dependency>,
    pub copied: Vec<(Dependency, ResourceId)>,
}
```

`build_from_schema()` 把报告写进 `reports/m1-build.json`，前端只渲染，不重新计算。

### P2：安装/归档服务

把“复制到 SimCityData、归档旧测试包、校验哈希”从手工命令提升为后端 `install.rs`：

- `plan_install(package, game_dir) -> InstallPlan`
- `apply_install(plan) -> InstallReport`
- 只允许新身份包；同 TGI 覆盖需显式确认；
- 自动归档旧测试包到 `test-archive/`。

### P2：游戏内对拍记录

后端生成一个 `comparison.json`：menu instance、mask、LOD、材质、包哈希、期望菜单名。  
用户只需在游戏里确认“看得到/看不到/崩溃”，把结果回填，不靠人脑记 TGI。

## 4. 前端能力（暂不接入）

工作台接入时只做**薄客户端**，所有业务逻辑仍在 Rust：

1. **BuildReadinessPanel**  
   调 `asset_build_validate`，显示闭包报告（mask、材质、模板、LOD、菜单字段），不自己解析二进制。

2. **MenuPresetPicker**  
   读取后端 `menu_profiles`，选择“公园/NightHouse”“地标”等；只传 profile id。

3. **BuildResultPanel**  
   显示 `M1BuildResult`：包路径、资源数、delta/full、hash、ClosureReport。

4. **DeployPanel**  
   调 `asset_build_install`，显示归档/安装/冲突确认；不直接写游戏目录。

5. **ComparisonPanel**  
   显示本轮 `comparison.json`，让用户记录游戏内结果。

前端模块建议：

```text
src/pages/studio/components/asset-build/
  BuildReadinessPanel.vue
  MenuPresetPicker.vue
  BuildResultPanel.vue
  DeployPanel.vue
  ComparisonPanel.vue
  api.ts          # 只做 invoke + 类型
  types.ts        # 与 Rust Serialize 契约一致
```

约束：

- 不复制 Rust 的闭包/TGI 逻辑；
- 不直接读写 package；
- 所有用户文案走 i18n（zh-CN/en-US）；
- 工作台现有冻结区不改，新增 parallel 目录。

## 5. 验收顺序

1. `cargo test -p fluffy-open-scp --lib asset_build` 全绿；
2. 两个 fixture 用 `build_asset` 生成包；
3. `check_lot_mask_closure.py` 两个包均 OK；
4. `check_model_material_closure.py` 两个包均 0 missing；
5. TGI 冲突扫描：与官方/DLC 零冲突；
6. 安装到 SimCityData；
7. 游戏内公园菜单对拍：可见性、贴图、碰撞/放置、保存重载；
8. 只有 7 通过后，才把 build/validate/install 接到前端工作台。
