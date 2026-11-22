# 原版飞艇与通勤飞艇停靠场预览诊断（2026-11-22）

对象：资源 `00000000:00000000:6BD73FAC`、地块 `00B1B104:43F0C000:3001966A`。
来源：实际安装目录 `D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData` 的原版 App/Game/Graphics/DLC0/EP1 包与自动发现的 EcoGame 包。没有部署或改写游戏包，没有运行游戏。

## 已复现的两个缺陷

1. 飞艇资源在 DLC0 的 `00B1B104:43F0C600:6BD73FAC`，选中模型 `BF45A31D`。slot0 `94332112` 是 256×512 DXT1 彩色贴图，已经成功解码；但 COLOR + FLOAT4 顶点声明被判为 tint（uvKind=2）。该材质没有 f32 参数表，前端 propModels 的 tintUsed 分支跳过普通 map 加载，实际又没有 tintTex/paletteTex，于是白模。修复：载荷分类同时检查实际材质参数表；无参数表且有 UV 的模型输出 uvKind=1。既有 mesh_uv_kind 几何判据保留。
2. 地块 bin0 的 `836A0617 / 259B7086 / E5D80819` 在 EcoGame 中仅有 Parent 与类别绑定。原解析器没有对这种无模型标记的记录追父模板，返回空。三者父模板分别为 `0AB4DFE1 / 0AB4DFE2 / 0AB4DFE3`；例如 E1 的显式包装 `DC01B946` 在 Game 包包含 Vehicle Models。修复：原路径失败后才沿 Parent 查找，设循环与深度保护，保留原始 resource_id。首选模型分别恢复为 `315F77E8 / CA265D8B / 821ADA65`。

## 验证与范围

只读测试 `package_service::prop_preview_audit::airship_preview_audit`：设置 SC_ASSET_ROOT 为上述目录，运行 `cargo test -p fluffy-open-scp --lib airship_preview_audit -- --ignored --nocapture`。修复前仅返回飞艇一个资源；修复后四个全部返回，全部首选模型可导出且有 slot0 贴图，飞艇最终分类断言为 1。

其他检查：prop_models 4 项通过；package_service 48 项通过、8 项忽略，1 项 export_writes_via_sibling_temporary_file 因 Windows PermissionDenied 失败。全库测试 436 通过、25 失败、60 忽略，失败包括写文件权限、构建 BoundingBox 契约等；未验证这些失败在基线是否同样存在，不宣称全量回归通过。

冻结的 property-editor 目录未改。未做最终运行界面的视觉验收，后端测试通过不等于已验收截图或游戏运行行为。变更需要重新编译并重启桌面后端才生效。
