# 原版建筑 DCD50C4F 缺失调查（2026-11-21）

## 引用身份与复现

- 建筑：`00B1B104:42E1C000:788A5CC7`，当前原版 Graphics 包；解析模型 `788A5CC8`。
- decal：`00000000:00000000:DCD50C4F`，normal 集合的 index 2 和 3 两次引用。
- 字典：Game 包 `00B1B104:C67FB185:1813DA18` 的 entry 15；材质 `4491DE3A`。
- raster：App 包 RW4 `2F4E681B:00000000:04B8ACC6`，128×128，可完整解码 RGBA；并非缺失/空白资源。
- 另两张 normal decal 也来自相同跨包链，因此一栋建筑可以整组破洞缺失，同时 Graphics 中的涂鸦照常显示。

原 `resolve_decal_textures`、`find_decal_raster`、`decode_lot_surface_png` 只查当前与已打开包。只打开 Graphics 和 Game 时，该建筑的破洞没有 PNG；前端遇到 png=None 返回 null，stageDecals 改用单元标记对象，因此会看到可选择的矩形/标签，却没有真正的 decal 纹理。

这是一条已经在本地原版数据上复现的加载缺口。没有读取用户截图那一刻的运行时包清单，不能仅凭截图断言当时具体打开了哪些包，也不能把其他所有 decal 缺失都归于同一原因。

## 几何与 GPU 排除检查

使用 `crates/sc-exporter/examples/native_decal_fixture.rs` 从当前包导出真实网格与单元，交给真实 `buildDecalBody`，配合当前 App 包解码的纹理做 Chrome WebGL 绘制：

- index 2：投影 1，729 个裁剪后顶点；index 3：投影 1，543 个顶点。
- shader error 0；两处投影均绘出纹理。
- `tmp/native-decal-repro/before.png` 是使用简化灰色建筑材质的诊断画面，不是游戏效果对拍，也不是修改资源后的游戏截图。

因此本案例不需要扩大 depth、移动原版 transform、改材质路由或强制给原生 decal 添加悬空预览。

## 修复范围

新增 `src-tauri/src/decal_dependencies.rs`：先走旧解析器；存在缺失 PNG 时，再查当前/打开包相邻目录及配置的游戏目录中的五个标准依赖包，在临时 PackageManager 范围内补解。只补成功恢复的缺失条目，保留原本成功解析的 DTO。

新增 `PackageManager::preview_scope` 复用已有包句柄并持有本轮额外依赖，生命周期仅限此次解码；不向全局包池或侧栏注册依赖，不消耗全局 64 包名额。原生 session 通过该新函数解析 decal；旧解析器内部逻辑保留。

本轮未改前端渲染、冻结 property-editor、原版包、游戏部署或存档。

## 本地来源 SHA-256

本轮实际读取的原版包位于用户指定的 live install/SimCityData，哈希保存在 `tmp/native-decal-repro/source-hashes.json`：

- App：`A290BC0D81FAF75C8A04E852CE39C859AD8651B64B10B03D6D8186393C2B4066`
- Game：`79A804494912D06B565B1EDE1CE183586D5C2E3D1AC9668E3D176518341C59E2`
- Graphics：`EAB5BBBE08B8EF7D5F7E4B0538523A2364C2CD28CA73E9C77A4CE03768E84555`

既有 dump 仅用于检索线索；以上资源链与几何从当前包重新读取。未宣称当前原版包与历史 dump、构建物或测试包字节等价。

## 回归命令

`OPENSCP_DECAL_GAME_DATA` 指向真实 SimCityData 后运行：

```powershell
cargo test -p fluffy-open-scp --lib decal_dependencies::tests::native_holes_resolve_without_opening_app_package -- --ignored --nocapture
```

测试检查旧解析器缺失、两次 DCD50C4F 与 normal 集合补全、已成功 PNG 保持不变、全局仍只有原来的两个包。前端定向渲染/变换测试 7 项通过。

**执行结果：真实数据回归 1 passed，前端 7 passed。** Rust lib test 首次直接启动报 `0xc0000139`（Common Controls 入口点）；按项目既有已验证流程复制测试 exe 到临时目录，并为副本嵌入 Microsoft.Windows.Common-Controls 6.0 manifest 后执行成功。没有改动应用构建配置或 Cargo 原测试产物。

本轮为后端源码改动，当前运行的开发应用须重新编译/重启后重新打开该建筑会话才能消费新 DTO；仅刷新前端不会替换已运行的后端。
