# .blend 导入：只允许 Asset Editor 原生 LOTM（2026-11-06）

## 硬性要求

用户指令（2026-11-06）：`.blend` 导入后项目里保存的模型格式**必须是 LOTM**，
不允许 GLB；导出逻辑必须与 SC5 Asset Editor 完全一致，不允许“移植一部分”。

## 之前为什么错

`mod_flow_assets::import_blend` 有两条路径：

1. 找到 `ny_facade` 时调用插件原生导出（GLB + LOTM）；
2. 找不到时回退到内嵌 Blender 脚本，只导出 GLB 预览 + 槽位 PNG。

安装包与仓库的 `resources/blender/` 只有 README，`ny_facade` 从未 stage，
所以实际永远走第 2 条：项目里只有 `assets/models/*.glb`，槽位是作者源贴图，
没有插件导出时的 palette 行位移、法线重编码、relief 重映射等处理。
预览端再把 `byMaterial` / `uvContract` 丢掉，最终就是整栋楼共用一个材质 +
错误 UV 域。

## 现在的约束

- `import_blend` **只走插件原生导出**。找不到 `ny_facade`、Blender 不可用、
  插件拒绝该 .blend（原工程 B4 网格/未应用修改器无法处理）时直接报错，
  不再生成任何 GLB 预览。
- `import_native_bundle` 只把 `.lotm` 写入项目的 `assets/models/`；
  插件同时写出的 GLB 留在临时目录并在导入后删除。工作台以 `source=lotm`
  打开，和大学/商业两个既有项目同一条渲染链（`uvKind=2`）。
- 缓存键提升为 `openscp.blender.2026-11-06.2`，旧的 GLB/slots 缓存全部失效。
- 导出前在**内存中的副本**上应用修改器（`native_export.py:apply_modifiers`）。
  `tall_office.blend` 的 `051` 带一个“按角度平滑”几何节点修改器，插件会以
  `原生导出暂不支持未应用修改器` 拒绝；应用后导出结果与手工导出逐字节一致。
  源 `.blend` 不会被保存或修改。

## 插件定位

`blender_service::addon_parent()` 依次查找：

1. `OPENSCP_BLENDER_ADDON`（可指向 `ny_facade` 或其父目录）；
2. `<exe>/resources/blender/ny_facade`（Tauri bundle 资源）；
3. `%APPDATA%/Blender Foundation/Blender/<version>/scripts/addons/ny_facade`。

仓库内的 staging 仍由 `scripts/stage-blender-addon.ps1` 完成：

```powershell
$env:SC5_ADDON_PATH = "D:\ea-games\simcity_offline\entity\SC5 Asset Editor 1.23.7"
pwsh -File scripts/stage-blender-addon.ps1
```

`src-tauri/resources/blender/ny_facade/` 是第三方资源，已在 `.gitignore` 中
忽略；发布前需确认再分发条款（见该目录 README）。

## Golden 对拍（已验证）

输入：`D:/ea-games/simcity_offline/entity/tall_office.blend`
插件：`SC5 Asset Editor 1.23.7/ny_facade`（Blender 5.1 headless）

```text
out.lotm  57,818,904 bytes  SHA-256 185412dd5057ff9020e7609dd27e15189d59102e83f131800b67535f4d9fa8b8
tall_test.lotm (用户用 Asset Editor 手工导出)
          57,818,904 bytes  SHA-256 185412dd5057ff9020e7609dd27e15189d59102e83f131800b67535f4d9fa8b8
```

诊断摘要（`out.native.json`）：`nativeMaterials=112`、`triangles=115162`、
`windows=2520`、`decorPanels=1962`、`schema=openscp.ny-native/1`、
`source=SC5 Asset Editor 1.23.7`。

复现命令：

```bash
"/d/blender/blender.exe" --background \
  --python src-tauri/src/blender_scripts/native_export.py -- \
  "D:/ea-games/simcity_offline/entity/SC5 Asset Editor 1.23.7" \
  "D:/ea-games/simcity_offline/entity/tall_office.blend" \
  /tmp/native-check/out.glb
sha256sum /tmp/native-check/out.lotm
```

Rust 端到端（addon 查找 → 导出 → 只落 LOTM → 容器校验 → golden 哈希）：

```bash
OPENSCP_BLENDER_ADDON="D:/ea-games/simcity_offline/entity/SC5 Asset Editor 1.23.7" \
OPENSCP_TEST_BLEND="D:/ea-games/simcity_offline/entity/tall_office.blend" \
OPENSCP_TEST_LOTM_SHA256="185412dd5057ff9020e7609dd27e15189d59102e83f131800b67535f4d9fa8b8" \
cargo test -p fluffy-open-scp native_blend_import_writes_lotm_only -- --ignored
```

该测试已通过：输出目录只有 `assets/models/<sha>.lotm`，没有 `.glb`。

## 自查：与插件的差异清单

现在导出逻辑**就是插件源码**（`ny_facade` 原样 vendored），Rust 只负责：

| 职责 | 位置 |
| --- | --- |
| 找 Blender / 找插件 / 后台执行 | `blender_service.rs` |
| 在内存副本应用修改器 | `blender_scripts/native_export.py` |
| 只持久化 LOTM + 校验容器 | `mod_flow_assets::import_native_bundle` |
| 工作台按 `source=lotm` 渲染 | `useWorkbenchAsset.ts` / `parseLotModelContainer` |

因此以下插件行为全部随源码生效，不存在“漏迁移”：

- `channels.face_state` / `material_state.recover` 的逐面状态恢复；
- `resources` 的 pack atlas / palette recipe；
- `appearance` 的窗户/夜景随机；
- `native_asset.recipe` 的 palette 行位移、颜色乘算、法线重编码、relief 重映射、
  body contrast、窗/装饰/内景分支；
- 几何装配（tessellate、window/decal 裁切、切线）；
- LOTM v9 容器（8 张 PNG 顺序 `[empty, normal, empty, empty, tint, palette,
  shader, interior]`、params、uvKind=2、diagnostic）。

如果后续要求脱离 Python 运行时做纯 Rust 重写，必须以上面的 golden LOTM 为
逐字节/逐语义基准，按模块迁移；在当前需求下，vendored 插件是唯一能保证
“与 Asset Editor 完全一致”的实现。
