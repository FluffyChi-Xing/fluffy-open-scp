# 地图绿地着色 + 可游玩区域/BOC——引擎侧分析（2026-10-01）

> 来源：shader 容器命名片段解析（本目录 shader-fragment-census.md 的延伸）+
> 社区 mod「外围区建造Mod」（BOC，Build Outside City）包体解析。
> 状态：绿地着色公式链**完整提取**（getGrassAmount 主体待补）；
> BOC 机制**确认**；可游玩区域定义**有结论**。

## 一、绿地着色公式链（OpenSCP 地图面板的对照基准）

引擎把"绿地"画在**地块下方的地面层**（非 lot 贴图本身），链路如下
（`lotCalcLightingLawn` / `textureLotBaseOverlayTerrainMaskLawn` 原文）：

```hlsl
// 1. 顶部俯视 UV（ecoMap 是城市级俯视图集，1024 宽的 1/4 象限）
float2 topDownUV = In.texcoord4.xy / 1024.0 * 0.5 + 0.5;

// 2. 草量 = f(生态图 RGBA, 噪声图, 法线)
float grassAmount = getGrassAmount(combinedEcoMap, topDownUV, bumpNormal,
                                   0, ecoMapValues, noiseMap, topDownUV);

// 3. 三色插值：干旱 ↔ 草地(×缩放)，再被污染通道拉向污染色
float3 basicGroundColor = computeBasicGroundColor(
    ecoMapValues,
    groundColors.mDryGroundColor.rgb,
    groundColors.mAverageGrassColor.rgb * groundColors.mGrassAverageColorScale.xyz,
    groundColors.mPollutedGroundColor.rgb,
    grassAmount);

// 4. 关键一步：平方（gamma 型提亮/对比增强）
basicGroundColor *= basicGroundColor;

// 5. 与地块自身颜色按 alpha 混合（lot 表面 alpha=1 处完全覆盖地面）
Current.color.rgb = lerp(basicGroundColor, Current.color.rgb, Current.color.a);

// 6. 场景光照（specStrength 0.2/0.25, gloss 0.06, specE 16）
```

其中（`computeBasicGroundColor` 原文）：

```hlsl
float3 groundColor = lerp(dryGroundColor, averageGrassColor, grassAmount);
groundColor = lerp(groundColor, pollutedGroundColor, ecoMaps.a);
```

### OpenSCP 地图面板差距分析

| 引擎要素 | OpenSCP 现状 | 缺口 |
|---|---|---|
| ecoMap 采样（城市生态图集 RGBA） | 地图面板已有生态图层（水/矿/油/土壤） | 需把生态图作为**可采样纹理**进入地面着色，而非图层叠加 |
| noiseMap 抖动 | 无 | 需噪声图参与 grassAmount（否则色带/平涂感） |
| 三色插值 + **平方** | 疑似缺失/线性 | **平方步最可能就是"绿地着色不对"的直接原因** |
| groundColors 调色板（区域调色板） | 未见读取 | 需从 region palette 读 mDry/mAverageGrass/mGrassAverageColorScale/mPolluted |
| `lerp(地面色, 地块色, a)` | 有 | 对齐 |
| SimCityLighting | 已有同款 | 对齐 |

**结论：绿地着色的逆向基本完整**——剩余小项：`getGrassAmount` 主体
（在 g40212004 组容器，15 处引用）与 `combinedEcoMap` 的组表来源。
实现验证无需游戏：按公式离线合成 ground 贴图与游戏截图对拍即可。

## 二、可游玩区域是怎么定义的

综合引擎侧证据与 BOC mod 机制：

1. **区域边界不在渲染/放置引擎层，而在脚本层**。证据：
   BOC mod（`packages/外围区建造Mod/`）不改任何引擎资源，只做两件事——
   ①替换**游戏核心规则脚本包**（`SimCity-Scripts_*.package`，type
   `0x08068AEB`，解压 6.3MB，内含 `SC_RULE_*` 全量规则表与编译 JS——
   建设范围检查在此层）；②投放**属性包**（`1_bRangeRemovals`，29 条
   `0xB1B104` 属性）与**RW4 模型**（`1_aaWHATHAVEIDONE`，4 属性+4 模型
   1.5MB，盒外道路/设施用）。
2. **可游玩区域 = 城市盒（city box）由脚本读取的调参/属性定义**，渲染与
   放置系统本身可作用于盒外任意位置（BOC 证明：盒外道路/发展无需引擎改动
   即可显示与放置）。
3. **崩溃/回滚代价**：BOC README 明示 Overplop 有 **rollback**（模拟回滚）
   风险——脚本放行后模拟状态与预期不符的代价，OpenSCP 做盒外建设时同样
   要处理（放置合法性与模拟一致性需自行负责）。

## 三、BOC 具体改了什么（本轮包体解析）

| 文件 | 内容 | 作用 |
|---|---|---|
| `SimCity-Scripts_272391411.package` | type `0x08068AEB` ×1（1MB→解压 6.3MB 规则脚本） | **替换核心规则脚本**：移除建设范围检查、放行盒外 plop |
| `1_bRangeRemovals.package` | `0xB1B104` 属性 ×29（145KB） | 范围移除相关属性（道路/市政的范围调参） |
| `1_aaWHATHAVEIDONE.package` | 属性 ×4 + RW4 模型 ×4（1.5MB） | 盒外放置用的新模型/属性 |
| `1_airships/JetPlanes/GermanTrain/RotatedTrain...` | 载具/装饰包 | 盒外世界的填充内容 |
| `SimRollerCompletePack` | SimRoller 工具包 | 区域编辑相关 |
| `OverplopSetup.bat` + OverplopModules | Overplop 附加（含 Rollback 恢复说明） | 处理脚本放行后的回滚问题 |

### 属性包逐条 diff（prop_dump 对比原版 Game 包，09-30）

- `1_bRangeRemovals` 的 29 条属性 = **29 个 Unit 资产定义**（G 40E1C000，
  PropertySemantic::Unit），每条 49~104 属性；
- **与原版同 TGI 逐属性比对：每个 Unit 恰好一处改动——`0x0C8EE92F`
  （= HASH_COST，Unit 造价，unit_cost_scan.rs 实锤）改为 2³⁰（10.7 亿）
  或 2²⁶（6711 万）**；
- 判读：天价 Cost = 防正常经济误购的**盒外放置专用占位单元**（由 mod 的
  工具/脚本放置），与"RangeRemovals"命名互证；
- `1_aaWHATHAVEIDONE` 的 4 属性 + 4 RW4 模型同理（盒外建筑外观）。
- **脚本包 diff（附加实锤）**：mod 的 `SimCity-Scripts_272391411` 与原版
  EcoGame 同名包（2014-05-27，版本序列最新）**解压后 MD5 完全一致**——
  该 mod 未修改核心脚本，"区域外可建设"全部由属性层完成（修正此前
  "替换核心脚本"的初步推断）。原版 EcoGame 目录还有 6 个更早版本的
  SimCity-Scripts（游戏更新历史累积，可做版本演进研究）。

## 二·补充：脚本包的引擎消费机制（09-30 RTTI 实锤）

SimCity.exe 对 type `0x08068AEB` 有 3 处 dword 常量比较（.text @
0x1c86e5/0x1c8732/0x1c93f2）= **引擎内置解析器的类型分发分支**。RTTI 泄漏
给出完整类名与来源：

- `EA::ArgScript` / `EA::Swarm::Parser`——**引擎内置 ArgScript 解析器**
  （与 shader 容器同一文本命令格式家族）；
- `GB::cEcoGameDescription` + `cAddEcoGameDescriptionCommand`——EcoGame
  规则包经 ArgScript 命令注册进 GlassBox 的 EcoGame 规则库；
- 源码路径泄漏：`GB_EcoGame/...UTFKernel/EASTL`（GlassBox EcoGame 模块）。

**结论**：EcoGame 包 = ArgScript 格式的规则描述（"unit rule"），引擎在加载
时用自己的解析器解释并注册——**不需要 JS/V8**（`EAWebkit.dll`=WebKit/
JavaScriptCore，仅 UI 网页；`1911.dll` 全量加密无宿主痕迹；exe 无 V8 标记）。
`SC_RULE_*`（1035 处）= ArgScript 注册的规则名表。

**blob 格式注意**：EcoGame 规则库为自有格式（null 分隔名字表 1035 条 + 数据段），
与 shader 容器的长度前缀流不同——shader token 解析器不适用，需单独逆向其帧格式
（首样本已存 `tmp/boc/scripts_main.bin`，SC_RULE_ 出现 1035 次）。

**BOC 逻辑 diff 方法**：BOC 的 6.3MB blob = 修改后的 ArgScript 规则数据库
全文。用 `tmp/parse_container_tokens.py` 的同族语法解析（长度前缀字符串流）
可切出规则定义清单；与原版规则库 diff 即得逻辑差异。原版规则库需从
清版安装/社区原版包获取（本机 5 主包无 type 0x08068AEB 条目）。

## 四、外围坐标系 / 地下资源 / 摄像机（引擎侧判定）

| 主题 | 判定 | 依据 |
|---|---|---|
| 坐标系 | **无独立"盒外坐标系"**：世界是统一连续坐标系，城市盒只是脚本层的逻辑边界 | BOC 不改引擎即在盒外正常放置/渲染 |
| 地下资源 | 资源数据（矿/油）存储于**区域图（region map）的生态/资源图集**，覆盖范围由区域数据决定；盒外是否有资源 = 区域数据是否覆盖，引擎采样逻辑与盒内相同（生态图采样同 §一） | 资源图层与 ecoMap 同为城市级俯视图集（§一 topDownUV 同源） |
| 摄像机 | 运动限制为脚本/调参层（.text cmp 链可静态分析，需 Ghidra 定位）；渲染层对盒外无特殊处理 | d3d9 层交通图无"摄像机相关"差异 |
| 区域外城市 Impostor | regionCityBuildings Impostor 全套（17 块）= 远景区域视图的城市代理管线 | fragment-census.md Region 节 |

## 五、待运行时项（与主表一致）

N1 矩阵、N6 zoom、N9 复现、A-G 绑定流——依赖出生跟随周期（工具就绪）。
