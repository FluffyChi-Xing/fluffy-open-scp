# M1 Property 回写

回滚基线：`6aa6ff6`。本模块在当前分支新增，与原有资产工作台、`mod_flow`、冻结的 Property Editor 隔离。新增的 `asset_build_m1_build` IPC 需显式调用；不会自动迁移已有 schema、替换工作台构建入口或安装到游戏。

M1 完成的是 **Property schema → IR → Property 二进制 → DBPF → 重读验证**。它保留完整源包的资源 TGI 和未修改资源的解压内容，改变指定 Property 的字段。输出仍用原 TGI，因此属于覆盖包，不是自动分配新身份的独立建筑。

## 分层

| 模块 | 职责 |
| --- | --- |
| `plan.rs` | 复用 M0 校验/IR，校验 M1 能力范围、patch 语法与模板策略；纯函数 |
| `patch.rs` / `value.rs` | 类型转换、前值条件、数组/空值/删除；不可变输入，返回新 PropertyFile |
| `processor.rs` | 延迟执行的 Property 插槽，编码并重读比较属性语义；无文件 I/O |
| `connector.rs` | DBPF 读取、确定性组装、按完整 TGI 重读对比；不理解业务属性 |
| `package.rs` | 源快照、处理器编排、内容摘要、构建报告 |
| `filesystem.rs` | 项目路径边界、临时构建目录、验证后发布与失败清理 |
| `commands.rs` | 独立异步 IPC 入口，耗时工作进入 blocking 线程 |

计划和处理器不会读取全局状态。仅文件输出边界为临时目录分配进程内序号；该序号不参与内容摘要。未来 Raster/RW4 处理器可以输出同样的资源字节，由 DBPF 连接器组装。

## 调用与示例

Tauri `invoke('asset_build_m1_build', { root, schema })`：`root` 是项目目录，`schema` 是下面 JSON 的字符串。错误返回 `{ code: "asset_build_m1_failed", message: <技术详情> }`；后续 UI 接入需按 code 提供 zh-CN/en-US 文案，不能直接把技术详情当界面文案。

```json
{
  "$schema": "openscp.asset",
  "schema_version": 2,
  "id": "willis-property-test",
  "kind": "building-asset",
  "reference": {
    "package": "assets/reference/willis-source.package",
    "unit": "00B1B104:40E0C000:8D007745",
    "unitTemplate": "00B1B104:48E0C000:4E01E311",
    "lotTemplate": "00B1B104:40E1C000:4E01E311"
  },
  "scene": { "document": "asset.lot.json" },
  "semantics": {
    "properties": [
      {
        "tgi": "00B1B104:40E0C000:8D007745",
        "hash": "0x0A8A41DA",
        "value": 140000
      }
    ]
  },
  "outputs": { "package": "willis-property-test.package" }
}
```

`reference.package` 必须是项目内的源包副本。`expected` 可选，填写实际源字段值后用于拒绝过期修改。支持的类型与 `sc-properties` 一致：bool、int32、uint32、float、string8/16、key、text、vector2/3/4、colorRGB/RGBA、bbox、transform。拒绝整数溢出、float32 溢出和非法 Transform 长度。

- 新增字段需要 `propertyType`；已有字段显式类型必须匹配。
- `index` 修改或删除数组中的一个元素；无 `index` 且 `array: true` 替换整个数组，可新增数组。
- `value: null` 表示 Empty 变体；缺少 value 表示非法 set。Empty 可恢复为有类型的标量/数组。
- `operation: "remove"` 删除字段或数组元素，同样检查 `expected`；不接受 value。
- Key 值使用完整 `TYPE:GROUP:INSTANCE`，Text 使用 `{tableId, instanceId}`；数组里的 Vector 仍是嵌套数组。
- 默认拒绝修改声明的模板、`48E0C000` Unit 模板、已知地标模板对，以及源包中被 Parent Key 引用的资源。明确设置 `semantics.allowSharedTemplate: true` 才可修改这些目标。外部包的完整继承图不在 M1 解析范围。

## 产物与验证

```text
<project>/.build/<build_hash>/
  plan/plan.json
  plan/im.json
  artifacts/property/<TYPE>-<GROUP>-<INSTANCE>.property
  packages/<name>.package
  reports/m1-build.json
```

`schema_hash` 包含 properties 和模板策略；`source_hash` 包含源包全部资源 TGI/解压内容；`build_hash` 包含上述输入及处理器版本。源资源变化会产生不同构建目录。同输入可复现并复用完全相同的产物，已有内容不一致时拒绝覆盖。

所有 Property 成功编码后才创建 staging。只在 Property 语义重读、整个 DBPF 资源身份/字节重读通过后，将 staging 目录发布到 `.build/<hash>`。未编辑资源按原字节复制，修改资源仅接受现有编码器能完整往返的源表示；有尾随数据、非规范编码或损失性解码时拒绝回写。源包不覆盖，原压缩布局不保留。报告成功不代表游戏内验收。

## 当前边界及商业地标工程

`D:\open-scp-mods\商业地标资产开发` 当前是 v1 工作台节点、`asset.lot.json`、LOTM 和六槽贴图。这些源文件未修改。M1 示例应另存为 v2 schema，并准备含目标 Property 的项目内 package；不能把现有 v1 节点直接传入。

`scene.document` 在 M1 只记录作者文档位置，不读取或编译其编辑内容。含 `scene.model`、非空 `scene.slots`、`unit_overrides`、`lot_bindings` 或 `menu` 语义输入的构建会被拒绝，避免静默丢掉尚未实现的转译。新 TGI 分配、依赖重定位、跨包闭包、资产文档转 Property、数值族联动、RW4/Raster 回写、工作台按钮接入及游戏安装/运行验收尚未完成。

M1 可先修改已有模型引用 Key、Unit 费用或 Lot 字段并产出真实包；Blender 网格/六槽材质的完整闭环继续依赖后续 RW4/Raster 处理器。

## 验证命令

```powershell
cargo test -p fluffy-open-scp asset_build --lib
cargo test -p fluffy-open-scp mod_flow --lib
pnpm exec vitest run src/pages/studio/components/asset src/pages/studio/components/mod-flow src/pages/packages/components/property-editor
pnpm check

# 可选真实样本：仅在临时项目中复制并测试指定包。
$env:SC_M1_PACKAGE = 'D:\ea-games\SimCity\SimCityUserData\EcoGame\SimCity-Scripts_287520926.package'
cargo test -p fluffy-open-scp real_willis_property_package_readback --lib -- --ignored --nocapture
```
