# 低代码资产管线与 Lot 资产 Schema（v1 确定稿）

> 状态：**规划/设计定稿**（未实现）。本文记录产品目标与 Property Editor
> 预期产出 `lot.json` 的 v1 格式规范，作为后续 schema 解析引擎、拖拽编辑、
> 构建打包三类开发的单一依据。取证日期 2026-10-06。
> 字段与 `src/api/tauri.ts` 的 `LotEditorSession`/`LotUnitDto` 一一对应，
> 与 `docs/re/props-effects-spawners-paths-engine-flow.md` 的引擎取证互为映射。

## 0. 产品目标（记录在案）

1. **Property Editor 的编辑能力 = 低代码编辑器**：编辑产物是
   **schema JSON**（`lot.json`），**不直接产出 RW4 或 package 文件**。
   JSON 是中间表示（IR）——可 diff、可手改、可版本管理，渲染器与未来
   写回器都只消费它。
2. **开发工作台（Code/模组工作台）= 资产 IDE**：新建资产/模组项目后集成
   lot 绘制、UI 编辑、资产绑定、EcoGame ArgScript 开发等能力，各自产出
   对应目标文件（raster file、RW4、脚本资源等）。
3. **构建按钮（工作台右上角）**：点击后先更新依赖管理 `package.json`
   （模组清单，见 note-mubbaul5 规划），再把项目各目录下的目标产物打包为
   **一个/多个 .package 文件**——用户像 IDE 一样「写源码 → 构建 → 出包」。
4. 前置条件：decal / props / effects / spawners 机制全部弄清
   （props-effects 文档持续更新中）；机制清楚一项，schema 对应字段的
   「TBD」位转正，拖拽式资产编辑能力随之解锁。

## 1. 总体管线

```
                       ┌─ 工作台源文件（git 友好）─────────────┐
 Property Editor ────▶ │ assets/lots/<name>/lot.json  (本 schema) │
 （低代码编辑器）       │ masks/*.png                    │
 lot 绘制 ───────────▶ │ masks/painted-layers.png       │
 资产绑定/ArgScript ─▶ │ scripts/*.argscript | bindings  │
 模型导入 ───────────▶ │ models/*.rw4 | *.glb           │
                       └───────────────┬────────────────┘
                                       │ 「构建」（右上角按钮）
                       ┌───────────────▼────────────────┐
                       │ 1. 更新 modRoot/package.json     │
                       │    （覆盖 TGI 清单/版本/产物表） │
                       │ 2. 目录产物 → 一个/多个 .package │
                       │    property 行写回（sc-properties）│
                       │    raster 写回（mask/纹理）      │
                       │    RW4 条目直拷/重打包           │
                       └────────────────────────────────┘
```

**关键分层**：编辑态（PE 内存中的 overrides/选中/图层）只属于「编辑器视
图状态」；**资产语义**只看 lot.json。渲染器（three 场景重建）与写回器
（DBPF 打包）是 lot.json 的两个独立消费者，互不依赖对方。

## 2. 文件约定

```
<modRoot>/
  package.json                    # 模组清单（依赖/覆盖 TGI/产物表，已有规划）
  assets/
    lots/<asset-name>/
      lot.json                    # 本 schema（单一真源）
      masks/lot-mask.png          # LotMask（RGBA8；A=LC4 权重，通道=LC1-4）
      models/                     # 该资产私有模型（RW4/GLB；未来构建时入包）
      thumbs/preview.png          # 可选预览图
```

- 文件名建议 `<asset-name>.lot.json` 亦可；目录名即资产 id。
- schema 版本：`"version": 1`（整数，破坏性变更 +1，解析引擎按版本分发）。

## 3. 值类型约定

| 类型 | JSON 形态 | 说明 |
| --- | --- | --- |
| TGI | `{"typeId":"0x00B1B104","group":"0x40E1C000","instance":"0x0A61112C"}` | 十六进制字符串（域内惯例，调试可读）；解析器同时接受十进制数 |
| 向量 | `[x, y, z]` | f32 数组，长度即语义 |
| 变换 | `{"matrix":[12 floats 行主序], "flags":15, "unknown":4.24}` | WPF Matrix3D 行主序 12 floats（与 `UnitTransformDto` 逐字）；`flags==15` 时 `unknown` = Scale（半宽语义） |
| 枚举 | 字符串 | 如 `"Near"|"Mid"|"Far"|"Max"`、`"Point"|"Spot"|"Line"` |
| 颜色 | `[r, g, b, a?]` | 0-1 线性或按字段注明 sRGB |
| 属性字段 | `"0x0DA76A05": <typed value>` | 对象键 = property hash 十六进制；值按 `UnitFieldDto.typeName` 定型（解析引擎按 sc-properties 类型表编码） |
| 外部引用 | `{"kind":"file","path":"masks/lot-mask.png"}` 或 `{"kind":"tgi","tgi":{...}}` | file = 项目内相对路径（构建时入包）；tgi = 游戏包内既有资源（覆盖/引用） |

## 4. 顶层结构（v1）

```jsonc
{
  "$schema": "openscp.lot-asset/1",
  "version": 1,

  "meta": {
    "name": "工业仓库",              // 展示名（缺省 = instance hex）
    "author": "",
    "created": "2026-10-06T00:00:00Z",
    "modified": "2026-10-06T00:00:00Z",
    "source": {                      // 可选：从游戏包 fork 的来源
      "tgi": {"typeId":"0x00B1B104","group":"0x40E1C000","instance":"0x0A61112C"},
      "package": "SimCity_Game"
    }
  },

  "lot":   { /* §5 地块层 */ },
  "model": { /* §6 建筑模型层 */ },
  "units": [ /* §7 六类 Unit */ ],
  "editor": { /* §8 编辑器视图状态（重建可忽略） */ }
}
```

### 4.1 完整最小示例（解析器对照基准）

```json
{
  "$schema": "openscp.lot-asset/1",
  "version": 1,
  "meta": {
    "name": "工业仓库",
    "source": { "tgi": {"typeId":"0x00B1B104","group":"0x40E1C000","instance":"0x0A61112C"} }
  },
  "lot": {
    "size": [96, 96],
    "tilePeriod": [8, 8],
    "baseTile": 8,
    "colors": [
      {"rgba": [0.28, 0.225, 0.183, 7], "authored": true},
      {"rgba": [0.2, 0.2, 0.2, 9], "authored": true},
      {"rgba": [0.3, 0.25, 0.2, 0], "authored": true},
      {"rgba": [0.75, 0.75, 0.75, 11], "authored": true}
    ],
    "borderColors": [[0.4,0.38,0.35],[0.4,0.38,0.35],[0.4,0.38,0.35],[0.4,0.38,0.35]],
    "borderWidths": [1.5, 0, 0, 0],
    "borderPatterns": [0, 0, 0, 0],
    "mask": {"kind":"file","path":"masks/lot-mask.png"}
  },
  "model": {
    "lods": [
      {"lod":1, "ref":{"kind":"tgi","tgi":{"typeId":"0x2F4E681B","group":"0x00000000","instance":"0x4A8F5EF6"}}},
      {"lod":2, "ref":{"kind":"tgi","tgi":{"typeId":"0x2F4E681B","group":"0x00000000","instance":"0x1A1F9B28"}}},
      {"lod":3, "ref":{"kind":"tgi","tgi":{"typeId":"0x2F4E681B","group":"0x00000000","instance":"0x1A1F9B28"}}},
      {"lod":4, "ref":{"kind":"tgi","tgi":{"typeId":"0x2F4E681B","group":"0x00000000","instance":"0x1A1F9B29"}}}
    ]
  },
  "units": [
    {
      "id": "light:0", "kind": "light", "visible": true,
      "transform": {"matrix":[1,0,0,0, 1,0,0,0, 1,0,0, 24,10,48]},
      "lightType": "Spot", "color": [1, 0.95, 0.85],
      "outerRadius": 22, "innerRadius": 6, "diffuse": 1,
      "cullDistance": "Far", "volumetric": false,
      "fields": {}
    },
    {
      "id": "decal:2:1", "kind": "decal", "visible": true,
      "transform": {"matrix":[1.85,0,0,0, 0,1.85,0,0, 0,0,1.85, 12.4,3.76,48.1], "flags":15, "unknown":1.85},
      "category": 1, "scale": 1.85, "depth": 3.87,
      "materialData": [0.8, 0, 0],
      "fields": {"0x0DA76A05": 1.4, "0x0DA76A06": 3.76}
    },
    {
      "id": "prop:0", "kind": "prop", "visible": true,
      "transform": {"matrix":[0.35,0,0,0, 0,0.35,0,0, 0,0,0.35, 30,0.35,60], "flags":15, "unknown":0.35},
      "bin": 1, "slot": 2, "resourceId": 144704709,
      "tree": {"descriptor": {"typeId":"0x00000000","group":"0x40002D00","instance":"0xC602CD31"}, "lodIndex": 0},
      "fields": {}
    }
  ],
  "editor": {
    "groups": {"decals": false}
  }
}
```

## 5. `lot` 地块层（字段 ↔ property key 映射）

| schema 字段 | 类型 | property key / 来源 | 说明 |
| --- | --- | --- | --- |
| `size` | `[w, h]` 米 | LotSize | 无 = 未授权（壳 lot） |
| `placement` | 12 floats | 0x0DB7FB17 LotPlacementTransform | 行主序；地面矩形取逆对齐 |
| `tilePeriod` | `[w, h]` | 0x0CCB7FD0 | 地面贴图周期，平铺数 = size/period |
| `baseTile` | int | 0x0CCB7FD6（推导 0x0CCB7FD2/FD3，缺省 8） | 底图格索引 |
| `colors` | 4×`{"rgba":[r,g,b,a],"authored":bool}` | LotColor1-4 = 0x0D02D586-89 | A = 图案/法线图集格号 0-15；`authored:false` = 引擎回退调色板 |
| `borderColors` | 4×`[r,g,b]` | LotBorderColor1-4 | sRGB；描边平色 |
| `borderWidths` | 4×float | LotBorderWidth1-4 | 边框带半宽；全 0 = 无边框 |
| `borderPatterns` | 4×int 0-15 | LotBorderColor.A | 边框带图案格号 |
| `overlayBoxOffset` | `[x,y] \| null` | 0x0CCB7FC9 LotOverlayBoxOffset | 地面 quad 中心覆盖 |
| `modelBBoxCenter` | `[x,y] \| null` | 0x00F9EFBA | 仅供诊断对照（引擎无渲染消费） |
| `mask` | 外部引用 或 `{"kind":"inline","data":"<base64 RGBA>"}` | LotMask raster（0x2F4E681C，残缺 TGI） | RGBA8：RGB 通道 = LC1-3 权重、A = LC4；未来 lot 绘制产出的就是这张图 |

`decalLight`（0x0DA76A05/06 破洞假内景光 [光强因子, 半径因子]）挂在
`lot.decalLight`（可选）——它是 lot 级材质参数而非单元字段。

## 6. `model` 建筑模型层

```jsonc
"model": {
  "lods": [                        // index 0 = LOD1；null = 该级缺失
    {"lod":1, "ref":{"kind":"tgi","tgi":{"typeId":"0x2F4E681B","group":"0x00000000","instance":"0x4A8F5EF6"}}},
    {"lod":2, "ref":{"kind":"tgi","tgi":{"typeId":"0x2F4E681B","group":"0x00000000","instance":"0x1A1F9B28"}}},
    {"lod":3, "ref":{"kind":"tgi","tgi":{"typeId":"0x2F4E681B","group":"0x00000000","instance":"0x1A1F9B28"}}},
    {"lod":4, "ref":{"kind":"tgi","tgi":{"typeId":"0x2F4E681B","group":"0x00000000","instance":"0x1A1F9B29"}}}
  ],
  "lodsKeyed": false              // true = 0x0D8C29C3 显式 key；false = 包装记录
}
```

- 来源链（引擎口径，见 props 文档 §1.2）：显式 key 0x0D8C29C3 → 包装记录 →
  Vehicle Models 0x0D897169 数组。fork 时后端把解析结果物化为 `lods[]`。
- **树 prop 的 descriptor 引用**（0x0C36D30D + 0x0D8C29CF）在 unit 上表达
  （§7），不在 model 层——树没有静态 LOD 集。
- `ref.kind:"file"` = 项目私有模型（`models/` 下），构建时作为 RW4 条目入包
  并回填 TGI（构建器负责把 file 引用改写为 tgi 引用并登记 package.json）。

## 7. `units` 六类 Unit

数组顺序 = 组件树显示顺序。`id` 规则与现有 `unitId()` 一致：
`"<kind>:<category>:<index>"`（decal 带 category）否则 `"<kind>:<index>"`；
**拖拽新增的单元由编辑器分配该 kind 的下一个空闲 index**，id 即稳定标识
（编辑层 overrides / undo / 手柄绑定都引用它）。

公共字段：`id`、`kind`、`transform`（§3）、`fields`（§3 属性字段，hash 键）、
`visible`（缺省 true；**编辑态**，见 §8 说明）。

### 7.1 light

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `lightType` | `"Point"\|"Spot"\|"Line"\|null` | |
| `color` | `[r,g,b]` | |
| `outerRadius` / `innerRadius` / `diffuse` / `length` | float \| null | |
| `cullDistance` | `"Near"\|"Mid"\|"Far"\|"Max"\|null` | |
| `volumetric` | bool \| null | |

### 7.2 effect（Swarm 粒子效果挂载）

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `effectId` | TGI \| null | 0x02A907B5 族主 id（`HasVisualEffect` 判定） |
| `enabled` | bool \| null | 0xBC enabled 列 |

（真实粒子渲染需 Swarm 解析器，独立工程；PE 阶段 = 标记 + 语义。）

### 7.3 decal

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `category` | int | 字典类别（0 基，UI 显示 +1） |
| `scale` / `depth` | float \| null | 盒体尺寸 |
| `materialData` | `[f32, f32, f32] \| null` | decalWorldDirection 系参数 |

机制注记（props/decal 文档）：体积盒 = 记录矩阵纯数据构造（基 =
R×scale×0.5×10），唯一判据 = 可见类别；破洞家族 raster 即光衰减掩码。

### 7.4 prop

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `bin` | int | ID 列 = 本箱资源清单 |
| `slot` | int \| null | Transform+Slot 列 |
| `resourceId` | int \| null | 原型资源 id（脚本资源表反查 RW4） |
| `scale` | float \| null | transform.flags==15 时与 unknown 同义 |

树 prop 附加：`tree` 对象 `{"descriptor": TGI, "lodIndex": 0-3}` =
0x0C36D30D + 0x0D8C29CF（cGraphicsInstancedImpostor 触发签名）。

### 7.5 spawner

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | TGI \| null | 0x0E1BAC61 Key |
| `count` / `countRandom` | int \| null | 0x0E715928/29 |
| `agent` | TGI \| null | 0x0F0E2BF1 小人外观引用 |

### 7.6 pathPoint

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `point` / `tangent` | `[x,y,z] \| null` | 0x0CAA68xx 族点/切线列 |
| `pointIndex` | int \| null | |

（完整路径段 = 同 entry 的点序列；未来路径编辑器在此之上成段。）

### 7.7 `fields` 通用属性字段

凡上文未升格为具名字段的 property 行，一律进 `fields` 对象
（hash 十六进制字符串键 → 定型值）。升格原则：引擎有明确渲染消费的字段
进具名字段；仅透传的进 fields。写回器按 sc-properties 类型表反编码。
示例：破洞贴花假内景光若未来降为单元级参数，即
`"fields": {"0x0DA76A05": 1.4, "0x0DA76A06": 3.76}`。

## 8. `editor` 编辑器视图状态（非资产语义，重建可忽略）

```jsonc
"editor": {
  "groups": {"lights":true,"decals":false,"props":true,"effects":true,
              "spawners":true,"paths":true,"lot":true,"model":true},
  "unitVisibility": {"decal:2:3": false},
  "camera": {"theta":0.34,"phi":1.2,"distance":42,"target":[0,0,0]}
}
```

组/单元可见性在此留痕（PE 图层开关不写 units[].visible，避免编辑态与
资产语义混淆）；解析引擎**必须容忍本节缺失**。

## 9. 重建规则（schema 解析引擎约定）

1. 读 `version` → 分发对应解析器（v1 只此一份定义）。
2. lot 层：`mask/colors/border/baseTile/tilePeriod/placement` 直接喂现有
   `refinedGround`/`groundCompose` 管线——它们现在消费的 session DTO 字段
   与 schema 字段一一对应，解析器产出 session 同构对象即可零改动复用。
3. model 层：`lods[].ref` → GLB/RW4 装配（file 引用直接读项目文件；
   tgi 引用走 PackageManager——与现在 `read_lot_editor_session` 同路径）。
4. units 层：按 `unitId` 建对象、按 `transform` 摆位、按 kind 装配
   （现有 `buildUnitObject`/`getPropModelObject`/`getTreeModelObject` 复用）；
   **缺失资产引用降级为标记锥**，与引擎「不可解即不渲染」口径一致。
5. `editor` 节缺省时全部可见、默认相机。

## 10. 构建管线（工作台「构建」按钮，未来）

1. 扫描 `assets/lots/**/lot.json`（及未来 UI 编辑/脚本资产），
   汇总「覆盖 TGI 清单」（source.tgi → 本资产）。
2. 更新 `modRoot/package.json`：版本、覆盖 TGI 列表、产物表（跨模组冲突
   检测靠它，见 note-mubbaul5 规划）。
3. 打包（复用现有 DBPF 写栈 `crates/dbpf` + sc-properties 编码器 +
   raster 写回）：
   - `lot.json` → 0x00B1B104 property 行（具名字段按 §5/§7 映射表反编码，
     fields 透传）；
   - `masks/*.png` → raster 条目（0x2F4E681C，残缺 TGI 惯例）；
   - `models/*.rw4` → 2F4E681B 条目；GLB → 暂不（需 RW4 写回器，资产-1）；
   - 脚本资源（ArgScript/EcoGame）→ 对应资源类型（机制待逆向清单见
     roadmap）。
4. 一个资产一个 `.package`（对齐官方 mod-airport.package 惯例），
   构建器可按 package.json 的产物表合并/拆分。

## 11. 与机制待清项的关系（schema 的 TBD 位）

| 机制 | 现状 | schema 位置 |
| --- | --- | --- |
| decal 字典 effect/变体判定 | P4-D1 待解 | `fields` 透传位 |
| spawner/paths 完整语义 | 部分取证 | 具名字段已留，TBD 补充 |
| Swarm 粒子效果本体 | 独立工程 | `effectId` 引用位已定 |
| ArgScript 资产 | 待接入 | `assets/scripts/`（本文 §10.3 预留） |
| 树季节 HSV/叶量 | descriptor 已解码 | 树 prop `tree` 对象扩展位 |

机制每清一项：schema 增补具名字段（+minor），解析/写回两侧同步，
TBD 位（`fields`）保持兼容——旧 lot.json 永远可被新引擎读取。
