# `path` 与 `spawners` 被游戏忽略的只读调查报告

调查日期：2026-11-04。调查范围：OpenSCP 构建器、SimCity 开发版反编译证据、零售/开发包属性对拍，以及 `大学-综合实验楼-资产项目` 的现有 JSON/包。没有启动游戏、没有写入 SimCityData、没有修改项目源文件。

## 结论

当前现象由两条独立机制造成：

1. **路径不是几何线条资源。** 游戏只有在 `0x0CAA680D/0x0CB00ED8/0x0CAA6832/0x0CAA6841` 四组几何列和 `0x0CC8FE7A` 的 PathEntry 引用同时成立，并且该引用能通过脚本表 `IndexFromPathEntryID(instance)` 解析时，才把路径加入运行时路径网络。仅把点、切线和 `path` Property 打进 package 不足以让小人使用它。
2. **Spawner 不是“小人生成器”。** `0x0E1BAC61`（名字/ID）与 `0x0E1BAC62`（Transform）在引擎中装配成 `cUnitModel::mUnitLocations` 命名位置表。它提供门口、停车位、演出点、镜头/UI 锚点等位置；小人是否出现、从哪里来、走哪条路由 Agent/Transport/ER2 和路径网络决定。随意增加自定义 ID 不会生成小人。
3. **构建器存在一个高概率的 PathEntry 分组错误。** 当前 `src-tauri/src/asset_build_m1/scene.rs` 将每个 entry 写成 `00B1B104:40E1C400:<instance>`；而消防局、热气球公园及已验证的大学包都使用 `00B1B104:40E0C400:<instance>`。`大学-综合实验楼-资产项目/asset.lot.json` 的 entry `03106C2E` 本身是对的，但如果最终导出沿当前代码写成 `40E1C400`，脚本 PathEntry 查找很可能失败，整条路径会被引擎跳过。

## 引擎消费链

开发版反编译资料（`docs/re/props-effects-spawners-paths-engine-flow.md`、`docs/re/unit-props-effects-spawners-paths.md`）给出以下链条：

```text
Lot Property
  ├─ path columns ──> SC::CreatePathsFromProps
  │                    ├─ point/tangent/index/pair 解码
  │                    ├─ entry.instance -> IndexFromPathEntryID
  │                    ├─ scope/connect 过滤
  │                    └─ CreatePath -> cUnitPath 网络 -> Agent 沿段行走
  └─ 0E1BAC61/62 ──> cUnitModel::mUnitLocations
                       └─ GetUnitLocation(name) 给脚本/演出/UI 查询
```

`CreatePathsFromProps` 对缺失或不可解析的 entry 返回跳过路径；正常游戏也不会绘制可见路径线，路径只是 Agent 网络数据。编辑器画出的样条线是预览层，不代表游戏已经接受该路径。

`0x0E715928`、`0x0E715929`、`0x0F0E2BF1` 在开发源码中属于 building variation（变体基值/变体数量/影响 bin），不是 spawner 的人数、随机数量和 agent 引用。零售 `SimCity_Game.package` 对 472 个含位置表的 lot 统计为：7761 个位置，三列均为 0 命中；因此把这三列写进资产不能制造小人，反而可能造成并行列错位。

## 金样本对拍

### 消防局 `00B1B104:40E1C000:E917279C`

证据文件：`tmp/fire_prop.txt`（由 `prop_dump` 从 `SimCity_Game.package` 读取）。

| 列 | 实测值 | 作用 |
|---|---:|---|
| `0x0CAA680D` | 6 点 | Hermite 控制点 |
| `0x0CB00ED8` | 6 条切线 | 与点一一对应 |
| `0x0CAA6832` | 0..5 | 点索引重映射 |
| `0x0CAA6841` | 0,1,2,3,4,5 | 三条 `(start,end)` 区间，端点包含 |
| `0x0CC8FE7A` | `03106C2E`, `C5A09B67`, `418A18B1` | 每条路径的脚本类型引用 |
| `0x0D9A4E54` | `F44F784A` ×3 | local scope |
| `0x0D9AA2BB` | `C186DA37` ×3 | road-connect |
| `0x0E1BAC61/62` | 15 个 ID/15 个 Transform | 命名位置表 |

### 热气球公园 `00B1B104:43F0C000:8D7A596C`

证据文件：`tmp/balloon_prop.txt`（从 `SimCity_DLC0.package` 读取）。

| 列 | 实测值 |
|---|---:|
| `0x0CAA680D` / `0x0CB00ED8` | 5 点 / 5 切线 |
| `0x0CAA6841` | `0,1,2,4`，即两条闭合区间 |
| `0x0CC8FE7A` | `03106C2E`, `418A18B1` |
| `0x0D9A4E54` / `0x0D9AA2BB` | local / road-connect |
| `0x0E1BAC61/62` | 14 个 ID/14 个 Transform |

两个金样本都使用 **`40E0C400` PathEntry 组**，且共享 `03106C2E`、`418A18B1` 等已存在脚本 entry；它们不是只靠坐标工作。

### 大学资产现状

`asset.lot.json` 当前路径为：

```json
{"entry":"3106c2e","scope":"local","roadConnect":true}
```

当前工作区中的 `build/reference.package` 只有 13 个 Property、2 个 RW4 和 1 个 LOTM，未包含目标大学 lot；它不是可用于判断最终导出闭合性的最终包。历史构建产物 `tmp/UniversityAsset.package` 的大学 lot `00B1B104:41F0C000:70002000` 已经包含：

* 4 个点、4 个切线、索引 `0..3`、区间 `0,3`；
* entry `418A18B1`、`03106C2E`、`664FD4BD`；
* local `F44F784A`、road-connect `C186DA37`；
* 3 个位置 ID `92137622`、`16A2F7B4`、`16A2F7B5`。

这证明“列被写进包”与“游戏能消费它”是两件事：闭合还取决于每个 entry 的脚本资源是否随包可解析，以及 entry 的 TGI 分组是否与游戏数据一致。

## 对当前构建器的具体核验

`src-tauri/src/asset_build_m1/native_profile.rs` 已正确使用几何列哈希，并定义 `PATH_ENTRY_GROUP = 0x40E1_C400`；`scene.rs` 也按该常量生成 `0x0CC8FE7A`。这与对拍样本形成明显差异：样本均为 `0x40E0C400`。因此应把它视为第一嫌疑点。

另一个闭合问题是资源清单：路径 entry 必须是一个真实的 `0x00B1B104` PathEntry property，而不是只在 lot 列中出现的 instance。现有构建器的校验注释声称会把 PathEntry 加入包，但最终包需要逐 TGI 验证，尤其要验证 `03106C2E` 在 `00B1B104:40E0C400` 下存在。

Spawner 方面，大学资产的三个 ID 是自定义位置名。它们可以被 `GetUnitLocation` 查询，但目前没有证据表明任何消防/大学 Agent 规则、演出 actor 或 UI 查询引用这三个名字；因此游戏不出现小人是预期行为，不是 Transform 解码失败。

## 建议的只读验收门槛

1. 对最终导出 package 列出并确认 `00B1B104:40E0C400:03106C2E`（及项目使用的其它 entry）确实存在；同时确认 lot 的 `0x0CC8FE7A` Key 的 type/group/instance 与它完全一致。
2. 确认 `0x0CAA680D`、`0x0CB00ED8` 长度相等，`0x0CAA6832` 索引有效，`0x0CAA6841` 为偶数个且每对 `start <= end`；scope/connect 数量等于路径数。
3. 不把 `0x0E715928/29/0F0E2BF1` 当作 spawner 人数字段写入；只写 `0x0E1BAC61/62`，并检查两列长度相等、位置名不重复。
4. 若目标是“指定位置出现指定小人”，应继续追 Agent/Transport/ER2 或 vignette actor 绑定；仅增加 spawner 位置不能实现这一目标。

## 证据边界

本报告的反编译结论来自开发版带 PDB/IDA 资料及仓库中已保存的函数级取证文档；静态分析没有声称观察到一次真实运行时调用。包对拍是实际读取的 DBPF/property 字节。当前没有对最终大学包做游戏内运行验证，因此“`40E1C400` 导致路径跳过”是高置信根因假设，需用最终包的 TGI 清单或游戏日志再闭合一次。
