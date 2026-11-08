# lotm→游戏文件唯一路线：SC5 Asset Packager 模型管线迁移（2026-11-08）

状态：**E2E 已跑通**。裁决人：用户（死命令）。

## 裁决链

1. 2026-11-08 死命令：lotm→游戏文件完整迁移 SC5 Asset Packager 唯一路线，弃用我方全部模型合成路线
   （先例：blender→lotm 半个月卡死，完整迁移 SC5 Asset Editor 一步解决；本次同理）。
2. 2026-11-08 范围修正：**迁移只解决模型**（几何/朝向/材质/RW4/地面栅格/打包）；
   **道具、灯、行为属性以我方 s31 GOLDEN 配方为准**（树/path/垃圾桶/车 bin/light 已实机验证）。

## E2E 证据

- 输入：`tall_test.lotm`（115,158 三角形 / 112 材质组，SC5 Asset Editor 迁移路线产出）+
  游戏提取 96x96 地块（裁剪至 dev 白名单 19 字段）
- 命令：`runtime/sc5_packager.pyc build projects/tall_test`（9 步编译 + 内置回读校验）
- 产出：`tools/sc5_asset_packager/projects/tall_test/output/tall_test_0.1.0.package`
  328,175,429 字节，sha256 `0098d572433f505cdd688a11dc06f50bfb67e2e362836b3dcf0203efb677e8e2`
- 校验：菜单/属性/Unit 费用/三角形数/单网格/未解析引用/内景图集/朝向 bounds 全部 roundtrip 过
- 注意 0.8.0 版几何策略拒绝该资产（Zero-area triangle）——**必须用最新版工具链**（用户提供的
  `SC5 Asset Packager (1).exe`，2026-11-08 18:37）

## 架构定位

```
我方 SC5 Asset Editor 迁移管线          人家 SC5 Asset Packager runtime/
  blender → lotm+glb  ──────────────►  backend_compile:
（已跑通，不动）                          几何转换/朝向/材质图集/RW4/地面遮罩/DBPF 打包
                                          （唯一模型路线，E2E 已证）
                                        build_candidate.main(property_profile_factory=…)
                                                  ▲
我方 asset_build_m1 属性模块             ─────────┘
  menu.rs / behavior.rs / behavior_nodes.rs
  native_profile.rs / path_catalog.rs …
  （s31 GOLDEN 已验证的道具/灯/行为/菜单字段）
  → 导出 {menu,unit,asset} 字段字典 JSON → factory 注入
```

处置：我方 `asset_build_m1` 的**模型合成阶段**（geometry_route 的 nh-atlas、materials、
native_model、material_atlas、single_compatible、raster、collision 等）由人家路线取代，
停用或删除；**属性阶段**（menu/behavior/property semantics/path_catalog）保留并新增导出器。

## 拆接契约（实现依据，`tools/sc5_asset_packager/PROVENANCE.md` 有完整版）

`property_profile_factory(native, model_bounds, allocator)` 返回
`{menu, unit, asset: {hash→recordHex 字典}, mask, baseTile, period, iconLabel, manifest}`；
`build_candidate` 自行覆写全部模型侧字段（LOD/bounds/mask 引用/图集/图标/locale），
禁含外部 parent `00B2CCCB`（与我方 parent-free 配方一致）。

## 工件与复现

- 运行时：`tools/sc5_asset_packager/runtime/`（最新版 exe 解包，零改动，PROVENANCE 逐模块台账）
- venv：Python 3.12.11（uv）+ numpy 2.3.5 + Pillow 12.3.0
- 输入准备：`scripts/extract_ground.py`（人家 package_io 跨包合并地面）+
  `src-tauri/examples/trim_ground_lot.rs`（我方裁字段，白名单来自反汇编 dev backend）
- 项目：`tools/sc5_asset_packager/projects/tall_test/`（project.json 含 backend.command 指向 .pyc）

## 已知边界（如实）

- `research_property`/`backend_native`/`native_prop_mesh`/`scan_effect_catalog`/测试模块不在 exe 内
  （作者开发区私有）；编译路线闭包不受影响。
- GUI 需要 Tcl/Tk（uv Python 无）；编译路线不需要 GUI。
- E2E 包内道具/灯/行为仍是人家 legacy 配方——仅模型验证用；正式资产必须走拆接后的我方属性管线。
- 未经 menu-load oracle / 一次性城市验证前，E2E 包不部署。
