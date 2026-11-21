# Decal 放置类型标签（2026-11-21）

## 判断依据

标签描述游戏材质的放置机制，不读取编辑器为未附着 decal 生成的预览，也不使用未知材质的 renderer fallback 推断游戏行为。

| 字典 Material ID | 已验证 shader 链 | 标签 |
| --- | --- | --- |
| `E5390A98` | `700000CD` decalProjectSDFLitFront：receiver depth 投影、完整盒裁剪及背面过滤 | 投影 |
| `4491DE3A` | `700000D2` decalInteriorMap：receiver depth 投影、完整盒裁剪；另用 decal 盒坐标采样假内景 | 投影 |
| `73684EFC` | `700000D1` decalNeonTubeSDF：中心面压平、NoClip 自持灯管；另有 `700000CF` decalProjectNeonSDF 投光相位 | 可悬空 |
| 缺失或其他 ID（含尚未核实 shader 链的 `44A3F5BA`） | 不猜测 | 待确认 |

证据：`docs/re/decal-engine-reanalysis-ida.md` §11–12 与已提取 HLSL。
本轮重新读取本地 compiled-state 二进制 `+0x18` 的 shader ID：
`tmp/dumps/cstate_4491DE3A_rt0.bin`=D2，
`tmp/dumps/cstate_73684EFC_rt0.bin`=D1，
`tmp/dumps/cstate_73684EFC_rt1.bin`=CF，
`tmp/cstate_E5390A98_rt0.bin`=CD。
这些是既有提取物，未拿它们与当前部署包宣称字节或运行时等价。

“吸附”容易暗示会自动寻找最近表面；当前已核实的实例创建函数直接应用 authored transform，投影 shader 只裁剪投影盒覆盖的 receiver，没有自动搜寻远处表面的证据，因此不新增“吸附”分类。
实际投影距离由实例 depth × scale 决定，字典条目没有统一的固定米数。
“可悬空”表示主体可悬空，不表示投光相位也能在没有表面的空中显示；原 HLSL decalBaseCenter 将可见主体放在体积中心面。

## 接线与验证

- `src/lib/decalPlacement.ts` 为独立纯函数，仅维护已验证 Material ID。
- `DecalDictionaryGallery` 通过 `showPlacementType` 显示每项右上角短标签和本地化悬停说明；只有资产编辑库启用，不改变其他图库视图。
- 中英文：投影 / Projected、可悬空 / Can float、待确认 / Unverified。
- 8 项定向测试通过，覆盖三种已知材质、未知材质、资产库开关接线、每条标签及活动 UI 的语言切换。
- TypeScript 与改动文件 ESLint 通过；Chrome 实际挂载图库，在 340px 宽四列布局中 12 个标签均位于条目右上角且未越界，截图 `tmp/decal-tags/check.png`。
- 未更改游戏渲染路径、放置变换或打包数据，未触碰冻结 property-editor 目录。
