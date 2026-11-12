# 开发版地形编辑器的离线移植核查

地图面板可以继承笔刷输入、图层数据和资源输出契约。开发版编辑器对象依赖原生 Swarm、渲染材质、GPU render target 与资源管理器。面板可复用这些算法的输入输出契约，直接产出 Windows 零售版已经会读取的地形资源。

## 开发版处理链

| 阶段 | 原版实现 | 面板移植边界 |
| --- | --- | --- |
| 记录笔刷 | [BeginAccumulatingBrushes](../source-code/SimCity-dev-ida.c#L213586) 按图层基名选择队列，[ApplyBrush](../source-code/SimCity-dev-ida.c#L213335) 保存位置、旋转、尺寸、纹理和描述，[EndAccumulatingBrushes](../source-code/SimCity-dev-ida.c#L207556) 结束该图层的累积 | 以区域 XY 表示笔刷落点，按顺序保留参数。沿用地图面板场景反变换。 |
| 求高度场 | [RenderHeightMap](../source-code/SimCity-dev-ida.c#L210170) 按顺序执行笔刷，[SetBrushMaterial](../source-code/SimCity-dev-ida.c#L207917) 绑定笔刷纹理和上一张高度 target，选择 Add、Subtract、Level、BiasedAdd、AbsoluteMin/Max。侵蚀另走 kernel 分支 | C++ 可确定调度、输入和材质名。逐像素移植还需要实际材质、笔刷纹理、采样和混合公式。先对拍原版一支 Add 笔刷。 |
| 生成区域资源 | [Save341HeightMaps](../source-code/SimCity-dev-ida.c#L213091) 选择图层并生成区域多级输出。[FinalizeGameHeightMap](../source-code/SimCity-dev-ida.c#L211285) 读回 CPU、构造 TGI 并写入 save area | 面板从原包读出 F0 原始像素，在同一坐标系编辑，再按目标 TGI 输出标准 DBPF。现有 writer 可做格式验证，其 mip 过滤需另行对拍。 |
| 生成城市资源 | [ExportCityHeightMap](../source-code/SimCity-dev-ida.c#L222364) 按城市槽位位置和尺寸采样地形，单独输出 `2k_terrain_ground` | 先用盒外点验证区域 F0。盒内修改还要核对城市资源的加载关系，并按原版城市范围与采样规则重建。 |

本机零售原包读回城市 `2k_terrain_ground` 为 `03E421F0:D41AC555:8238C400`（1026）和 `03E421F0:FCF2DBF8:8238C400`（1027），各 `20 + 256×256×2` 字节。它们与区域 `0xBEAF0510` 的 F0 金字塔属于不同 group。同类型只能说明载荷编码一致。

## 地图面板与现有写入器

[地图面板](../../src/pages/studio/panels/map.vue#L125) 调 `map_panel_region_3d`，已复用地图预览组件。编辑落点可沿 [sceneToMap](../../src/components/map/map-coordinates.ts#L8) 回到原生地图 XY。后端已有 [read_region_field](../../crates/sc-properties/src/region_write.rs#L126) 读取 `4096×4096` 区域原始高度，[build_heightmap_overlay](../../crates/sc-properties/src/region_write.rs#L298) 按源头重建 341 张 F0 并复用 20 字节头。[map_panel_write_heightmap](../../src-tauri/src/map_panel.rs#L318) 已注册但当前地图面板未调用，入口消费整幅 16-bit PNG。Mod Flow 已直接复用相同的 Rust writer。面板接入可把笔刷操作交给独立计算模块，预览与导出读取同一份结果，避免用预览图的颜色或裁切像素写包。

## 白水谷实包离线对拍

原始 `SimCity_RegionTerrain0.package` SHA-256 为 `2ff8bcac2272b83ef05fba23d3d775402c2de8b7e6d5a4383f6a22f261c6d4a6`。使用现有 writer 写了两个工作区临时文件。一个直接重建原始场，SHA-256 `70e9549e301713f0f60daaa73aa928acd704f77a75cda68330fbc9a9808f912f`。另一个仅在区域像素 X `2312..2319`、Y `2064..2071` 的 `8×8` 范围增加 320 raw 单位，SHA-256 `5cd663f29efa286d9bdce56cacbf5fb4ce2ac5e4bbbdcdcdf6916f967b4a1e9b`。这是 writer 对照用的固定输入，原版 Add 笔刷输出尚未对拍。

两份包均回读 341 条 F0，TGI 和头与原包匹配，编辑包 mip0 的整幅像素与输入逐值相同。以下按 mip4 至 mip0 排列。

| 比较 | 变化 tile 数 | 附加结果 |
| --- | --- | --- |
| 原包 → 无编辑重建 | `1,4,4,16,0` | 25 张粗 mip 有变化，最大单像素绝对差依次为 `4750,3078,1516,923,0` raw。 |
| 无编辑重建 → 固定增量重建 | `1,1,1,1,1` | 每级只有一张受影响，变化像素数为 `1,1,4,16,64`。 |

因此当前 writer 已证实能生成可解析的目标高度图资源。直接重建整组会带入与编辑无关的粗 mip 差异。首个实验应从源包保留未修改资源与未受编辑影响的粗 mip 像素；父 mip 的更新应以原版过滤对拍或明确标注的局部探针方案为依据。导出前用零改动回读和逐级差分检验实际改动范围。

首个实机验证可使用白水谷 1027 东侧盒外的区域高度点。先核验原包与待部署包哈希和台账，确认存档隔离与备份，再按项目规则部署。由用户冷启动确认主菜单，进入一次性城市后核对地形和跨缩放切换。此次仅做源码与实包的离线检查，没有实现笔刷 UI、启动游戏或部署临时包。
