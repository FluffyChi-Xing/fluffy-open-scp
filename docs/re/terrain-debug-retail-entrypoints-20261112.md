# 地图 Debug 工具离线复核（2026-11-12）

本轮验证区域编辑环境开关在当前离线零售版的实际作用，并核查开发版 `DebugModifyMap` 的回放分支。

## 样本

- 零售 `SimCity/SimCity.exe` SHA-256 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。用 pefile 和 Capstone 读取磁盘 PE，以下零售地址为 VA，映像基址 `0x400000`。
- 开发版使用仓库 [IDA 反编译导出](../source-code/SimCity-dev-ida.c) 和 `tmp/dev-sample-analysis` 下的 `SimCity.unwrapped.exe`（SHA-256 `03045b870a3b3c405af72e0cd21c07aed1dfa9f08c0b8dd12aece4bcbf1f056e`）。用户指定目录的 `SimCity.c` 只有 `#error "A8EC50: INTERR: 73 (funcsize=0)"`。

## 区域编辑环境开关

开发版在 [环境读取](../source-code/SimCity-dev-ida.c#L225396) 后把 `0x0E276F5D` 的布尔值直接交给 [SetRegionAuthoringActive](../source-code/SimCity-dev-ida.c#L207150)。[Height](../source-code/SimCity-dev-ida.c#L227090) 在开关开启时尝试 `PointSampleHeightInRegion`，失败则回退到 `cTerrainStrata::Height`。

零售 `.text` 中查得一处 `0x0E276F5D` 立即数字节。`0xBE86B4` 查询该属性，`0xBE86D0` 写入对象 `+0x27995`，`0xBE86DC` 将该值 OR 到 `+0x27993`。这条属性读取路径随后查询两个应用属性并返回。开发版在对应位置的相机切换虚调用没有出现在该零售路径中。

零售 `0xBEC090` 读取 `+0x27995`。开关开启，或关闭但任一输入轴的绝对值大于 `1024.0`，都会尝试调用 `0xBEBFE0`；返回值大于零时采用输出高度，否则回退到 `0xBE6720`。`0xBEBFE0` 先分别给 X/Y 加上对象 `+0x2797C/+0x27980` 的浮点值，再检查区域范围并查找采样位置。这提供了一个可以继续追踪的城市到区域坐标对照点。`+0x27993` 另被 `0xBEB888`、`0xC051AA`、`0xC0524E` 读取，三处均可能走同一个对象的 vtable `+0x18` 查询，接口身份待核实。

## DebugModifyMap 与交互入口

开发版 [回放 switch](../source-code/SimCity-dev-ida.c#L651387) 将 `kLogActionDebugModifyMap` 和 `kLogActionDebugSetBin` 合并为清理并返回 `4` 的分支。[共享清理尾部](../source-code/SimCity-dev-ida.c#L651915) 也返回 `4`。这条开发版回放路径未调用地图修改。零售版动作分派仍需独立核对。

### 零售 `CreateTool` 的拒绝路径

当前零售版 `CreateTools` 在 `0x89C5A0` 调用 `0x89BF00`，传入刚取得的 action key instance。在 `0x89C5BF` 检查返回值，空指针跳过工具初始化和注册。工厂的 12 个 ID 与[既有零售普查](debug-tool-census-20261024.md)一致。用先前从 `454041B4` 读出的 `0x2A9AA359` 走比较树，依次经过 `0x89BF19`、`0x89BF2A`、`0x89BF3B`、`0x89BF46`、`0x89BF4D`，最后跳到 `0x89C293`，在 `0x89C296` 清零返回寄存器。修改菜单 Parent 和分类无法改变该分支。本轮复核的是 EXE 及既有 action 记录，未重新普查菜单包。

### 零售地形系统虚表对拍

从 MSVC RTTI 的 `.?AVcTerrainSystem@SC@@` 查找主对象 Complete Object Locator，再找指向它的虚表前置指针。开发版 locator 为 `0xCD8418`，主虚表为 `0xC31E64`。零售版分别为 `0xDA36EC` 和 `0xCF2458`。下表偏移以虚表首个函数指针为起点。

| 项目 | 开发版 | 当前离线零售版 |
| --- | --- | --- |
| `Init`（`+0x0C`） | `0x4052E0`，构造编辑器并注册图层 | `0x4051C0`，函数体到 `0x40521C` 返回，调用序列与开发版不同 |
| `Update`（`+0x18`） | `0x405490`，更新脚本编辑器和地形编辑器 | `0xC1E7C0`，`ret 4` |
| 编辑激活与编辑器接口 | `+0x24` 到 `+0x3C` 含地形/区域 authoring 开关及脚本编辑器入口 | `+0x24` 是 `0x405260`，取地形对象 `+0x27968`，随后虚表结束。紧接着 `0xCF2480` 是 UTF-16 字符串 `TerrainForServer` |

在当前零售 PE 的 `.rdata/.data` 未找到开发版 `cTerrainEditor` 或 `cTerrainBrushEffect` 的对应 RTTI 名。其 `.text` 也未找到开发版 `RunScriptBrushes` 使用的三个属性 ID 的原始小端序列（`0x0DBA3A9C`、`0x0DC097E3`、`0x0DE43899`）。这些是有范围的负面结果。结合虚表与空更新函数，可以确认开发版的交互编辑接口没有留在这份零售版的 `cTerrainSystem` 上。旧报告把开发版提取的 [RunScriptBrushes](../../tmp/dynamic/source-tree-impostor/src/SC/RunScriptBrushes.c) 当作当前零售版实现的判断需要撤回。发行包里的 Swarm 笔刷描述仍是数据证据，当前零售 EXE 对 type 32 的消费路径还需独立定位。

### UI 命令入口

基包提取的 `scrui-7D14BE70.js` 通过 `PostGameCommand("RunCheat", body)` 进入通用命令分发。对零售分发器 `0x8C73F0` 的 `0x8C7423–0x8C80CF` 线性反汇编，`runcheat` 的小写 FNV1 `0xF3D67CA5` 命中 `0x8C803F`，走 `0x8C8097 → 0x8C2F10`，非空 body 经 `0x405DD0` 返回对象的虚表 `+0x20` 转交。候选 `terraineditor`、`terrainedit`、`terrainauthoring`、`regionauthoring`、`regionedit`、`runterrainscript`、`debugmodifymap`、`setterrainauthoringtoolactive` 的哈希未命中该范围内的 `cmp eax, imm`。此检查覆盖直接分发比较，通用 `RunCheat` 的二级 ArgScript 注册表仍待核查。

## Package 能迁移到哪一层

开发版 `cTerrainSystem::PreInit` 在 `0x405619` 调用 `RegisterEffectTerrainBrush`，在 `0x40561E` 调用 `RegisterEffectTerrainScript`。前者的 `0x411830` 把原生 `Create`、`Read` 函数指针及版本范围 `2..3` 写入 `sComponentTable[32]`。后者给 type 33 写入另一组回调。Package 中的 Swarm section 是这些函数的输入。开发版 [ApplyBrush](../source-code/SimCity-dev-ida.c#L216711) 再把笔刷参数交给 `cTerrainEditor`。

零售版 `0xA8C744` 用 `type * 0x2C + 0x10356A0` 定位原生 `Create` 槽，槽为空则跳过创建。集合加载代码在 `0xA51FC2` 按相同步长读取 `Read` 槽 `0x10356A4 + type * 0x2C`，读取器为空或版本超出范围时，`0xA51FEC` 按 section 长度跳过。可用 type 1（`0xA4E53A`）和 type 2（`0xA88C80`）的直接写入复核表起点与槽宽。type 32 的 `Create/Read` 地址为 `0x1035C20/0x1035C24`，type 33 为 `0x1035C4C/0x1035C50`。这些槽落在 `.data` 的零初始化虚拟尾部。扫描零售 `.text` 未找到两槽任一字段的直接地址引用，零售 `cTerrainSystem::PreInit` 也没有开发版的两次注册调用。这给出了当前零售版跳过未注册笔刷的机制。间接注册和运行时槽值仍未实测。

零售版保留了成品 tile 的读取链。`0xBEB520` 使用 `ecomap_x%02d_y%02d_mip%d` 构造名字，计算小写 FNV1，输出 type `0x03E421ED` 和传入 group。`0xC05244` 的资源管理器虚调用查找该类型，成功后在 `0xC05288` 将资源像素送入 raster 填充。这一段和开发版 [tile 读取代码](../source-code/SimCity-dev-ida.c#L231237) 对应。开发版若能产出与目标 group、tile 索引、mip 和像素格式匹配的成品，可以作为零售加载实验的输入。OpenSCP [已具备 ED 读取与预览](package-validation-and-map-edit-dependency-closure.md#43-地图高度地面资源与资源画刷)，ED 写入尚待实现或验证。

交互编辑器需要工具创建、笔刷组件注册、编辑状态和写回链配合。提取菜单、贴图和笔刷配置只会取得资源部分。独立编辑器可在游戏外生成零售已能读取的 `0x03E421F0` 高度和 `0x03E421ED` 生态 tile。`0x03E421ED` 载入验收针对区域图层。城市煤、油、矿和地下水的模拟初值还需核对 [bootstrap 与 typed map](region-load-road-resource-lifecycle.md)。

## 零售 RunCheat 二级注册与区域 tile 原包对拍

`0x41AB5D` 创建 CheatManager，`0x41AB71` 通过对象虚表 `+0x0C` 初始化，`0x41AB76` 将该对象设为 `0xE4EFF4` 的全局实例。对应虚表 `0xCFCD80` 的 `+0x20` 是 `0x46D1B0`。该函数在 `0x46D236` 把命令正文交给对象 `+0x44` 的 ArgScript parser 虚表 `+0x58`。初始化函数 `0x46BEF0` 构建该 parser，并调用 `0x46B4A0` 注册 `help`、`history`、`clear`。CheatManager 的 `+0x18` 注册方法 `0x46D4F0` 调用 parser 的 `+0x18`，传入命令名与 handler。

沿全局 getter 和已确认的注册虚表槽，核对到 `picture`、`option`、`prop`、`movie`、`quit`、`hiresCapture` 等名称，`0x63805A` 还有 `framemetric` 的独立注册调用。基包 UI 的 `test RestartSCBox` 字符串仍在，但在当前 EXE 的 `.rdata/.data` 未找到 `test` 或 `RestartSCBox` 的独立 C 字符串。此轮没有找到地形编辑命令的注册证据。检查覆盖已识别的直接注册点和这些候选名字，间接注册、别名及运行时解析器的完整命令表仍未穷举。

对实际安装的 `SimCity_RegionTerrain0.package`（SHA-256 `2ff8bcac2272b83ef05fba23d3d775402c2de8b7e6d5a4383f6a22f261c6d4a6`），用仓库 DBPF 读取器按白水谷 group `0xBEAF0510` 逐条解压。两套资源各 `16² + 8² + 4² + 2² + 1² = 341` 张，全部按原生名字的小写 FNV1 命中。同一资源族的 20 字节头在全部 tile 中一致。

| 原生名字 | 完整 TGI | 解压后大小与像素载荷 |
| --- | --- | --- |
| `ecomap_x08_y08_mip0` | `03E421ED:BEAF0510:A89FF624` | 65,556 字节，20 字节头 + 128×128×4 |
| `heightmap_x08_y08_mip0` | `03E421F0:BEAF0510:D8423E2C` | 131,092 字节，20 字节头 + 256×256×2 |
| `ecomap_x00_y00_mip4` | `03E421ED:BEAF0510:10E6E1C0` | 区域根 mip，同样 128×128×4 |
| `heightmap_x00_y00_mip4` | `03E421F0:BEAF0510:7ADCBE88` | 区域根 mip，同样 256×256×2 |

这份原包的 ED 第四字节（A8R8G8B8 的 A）在 341 张 tile 中均为 0。对开发版成品做兼容检查时应保持目标区域 group、完整 tile 命名、同一 `(x,y,mip)` 的高度/生态配对与原生像素头。首个局部实验若涉及跨缩放观察，还需要沿变更范围更新父 mip。上述为原包读取与格式基线，尚未对拍开发版新导出物或验证改包加载。本轮没有启动游戏、修改游戏目录或部署包。

## Reddit 游戏内区域编辑演示与开发版能力

silkiokas 公开了[游戏内区域创建器的早期预览](https://www.reddit.com/r/SimCity/comments/1w3mrkx/comment/pbm1zoc/)。[原帖](https://www.reddit.com/r/SimCity/comments/1w3mrkx/13_years_later_simcity_2013_can_run_fully/)说明开发测试用 macOS Aspyr App Store 版，展示定制地形和水图。作者在[实现范围说明](https://www.reddit.com/r/SimCity/comments/1w3mrkx/comment/p7eg23n/)中表示大地图涉及多项核心系统修改，并考虑另发自定义区域 package 和区域创建工具。公开说明未说明 Terrain Authoring 各按钮实际调用的函数。

- 开发版 [ApplyBrush](../source-code/SimCity-dev-ida.c#L213335) 接收位置、角度、尺寸和笔刷参数，排入当前图层；同处还有全域和带状笔刷。 [TerrainEditorSpecialDraw](../source-code/SimCity-dev-ida.c#L242566) 在 authoring gate 开启时按当前图层绘制。 [Save341HeightMaps](../source-code/SimCity-dev-ida.c#L213091) 支持高度图分块写回。
- [区域导出调度](../source-code/SimCity-dev-ida.c#L223917) 对土壤、地下水、森林层设置区域生态图导出和 reload 标志。 [城市配色生态导出](../source-code/SimCity-dev-ida.c#L222411) 还会按城市位置计算并单独写出 `2k_terrain_waterTable`；[城市资源导出](../source-code/SimCity-dev-ida.c#L222488) 另有油、煤、矿层。
- [SaveCityEcoMap](../source-code/SimCity-dev-ida.c#L221362) 将按名字导出的城市图写成 `0x03E421F0`，group 取城市组，instance 取小写 FNV1。离线重算 `2k_terrain_waterTable` 为 `0xDBCABF86`，`waterTable` 为 `0x0F92DC56`。另有按 `mEcoMapID` 写出高度相关生态图的[独立分支](../source-code/SimCity-dev-ida.c#L221996)。[城市模拟初始化](../source-code/SimCity-dev-ida.c#L690646) 以 `0x07E0AE06` 指向的图像 Key 加载 EcoMap；演示中水图变化还需结合实际绑定、模拟读数和存档重载核验。

开发版函数能解释演示中的地形与资源画刷效果，当前 Windows 零售样本的工具工厂和编辑虚表缺口见上文。Aspyr 样本及作者插件均未取得，因此仍无法判断作者直接调用残留的原生编辑能力、补回原生调用链，或自行实现其中部分。
