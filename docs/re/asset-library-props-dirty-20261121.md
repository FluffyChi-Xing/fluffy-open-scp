# 资产组件库、prop 稳定性与保存状态（2026-11-21）

## 1. 组件库范围与加载

移除 UI 的杂项以及命名模型目录兜底，不再并发请求 `listModelCatalog`。
`list_prop_bin_catalog` 改用 `supported_prop_catalog`：固定树 8、车辆 3、垃圾类 9、牌子 3 个 resourceID；只对这 23 个 ID 执行既有资源→模型链。
垃圾类固定 ID 位于 `supported_prop_ids.rs`，另外三类沿用已验证的树/车辆常量及牌子三变体。
本地原版数据对比旧完整目录的非 Misc 集合，23 个 resourceID 完整一致。
资源 ID 固定，模型 TGI 仍从对应资源链解析并核实实际包所属，避免把会话包句柄硬编码进资产。

## 2. Decal 分类与重复目录

旧 UI 按 packageId 展示，而后端每次打开同一路径均可分配新 ID；`resolve_prop_models` 重复注册 Game/Graphics，因此同一个包会出现多行、重复解码相同字典并占满包池。
修复：

- `PackageManager::insert` 在锁内按规范路径复用已有句柄，检查重复先于包池限额。
- 后端 decal 目录按规范路径去重后再扫描。
- UI 以完整 atlas Group 为根分类；同一字典 TGI 合并，来源句柄仍传入图库/拖放，不作为分类标题。
- 树和牌子多资源共享模型，卡片 Vue key 加入 resourceId，避免共享模型的卡片被错误复用。

## 3. Prop bin 间歇缺失

确认并修复的独立故障：

1. `finish` 先记第一个候选模型的包 ID，再按模型大小排序；排序后首选模型可能属于另一个包，读模型自然失败。现在排序后绑定实际拥有首选 TGI 的包，并写出实际 group。最小双包测试可在旧路径复现错误，无需游戏安装。
2. 每次 `resolve_prop_models` 无条件释放上一批 EcoGame 句柄，可能使另一视图尚在加载的返回 ID 失效。取消这一无条件释放；保留已有容量约束和显式生命周期机制。重复 Game/Graphics 注册同时通过路径身份复用消除。
3. 工作台 prop 加载 watcher 原来仅以 resourceID 集合作键；切换到包含相同资源的另一资产时，旧结果被 epoch 判过期，却不一定触发新加载。现在键包含响应式场景 epoch。
4. 组件库原来只有 modelPackageId，没有持久路径；新条目带 modelPackagePath。旧无路径文档的失效/缺失句柄会重新按语义 resourceID 解析，载荷仍绑定到原单元的渲染缓存键。

这些修复针对资源身份、异步生命周期与持久引用；未扩大模型、随意更换参数或把缺失模型假装成成功。用户截图所用项目未在本轮重新打开，不把定向测试表述为该项目全部 prop 的运行时验收。

## 4. 保存状态

原逻辑保存时部分基线来自带缩进 `schemaJson`，watcher 却使用紧凑 `JSON.stringify`，同一内容也会不等；图层可见性同样进入了比较。
统一由 `workbenchContentKey` 计算内容身份，排除 editor.groups、editor.unitVisibility、editor.selectedId 和 units.visible；所有保存/恢复基线均用相同函数。
dirty 改为与基线实时比较，因此还原真实内容后也会恢复干净状态。视口仍可消费即时显示设置，不再将它们视为资产内容编辑。

## 验证

- 前端：13 项定向测试通过，覆盖支持分类限定、decal Group/TGI 去重、旧句柄恢复、保存内容与视图设置分离，以及此前 decal schema→渲染路径。
- TypeScript、改动文件 ESLint、差异空白检查通过。
- Rust：双包首选模型归属测试通过；真实安装目录 23 项与旧目录非 Misc 集合完全相等，每个首选 TGI 都存在于返回包中且有 MESH 节；同一路径额外 insert 12 次不增加包数。
- 此前原版破洞未手动打开 App 包的回归再次通过。
- Windows lib test 仍按既有方法为临时 exe 副本嵌入 Common Controls 6 manifest，不改变生产构建产物。

后端改动需要重新编译/重启应用。未改冻结 property-editor 目录，未修改游戏包或存档。
