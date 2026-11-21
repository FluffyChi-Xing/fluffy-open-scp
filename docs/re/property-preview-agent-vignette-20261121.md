# Property 预览语义调查（2026-11-21）

## 1. 载具／小人空模型：截图 E94B4027

只读检查安装包 SimCityData/SimCityDataEP1.package 中 00B1B104:61F0C600:E94B4027。
记录包含 17 个属性，无 Vehicle Models (0D897169)、无 RW4 key、无 Parent。
0BEE4AB0 / 0BEE4AB3 指向效果 CD400AC9；0D9A718B 是 sink ID 0186EE3A。
引擎 SC/cGraphicsGame/TransactionAddAttachments.c 读取 0BEE4AB0 作为 effectKeys；
UpdateEffectOnSinkChange.c:34-46 从 TransportEntryFromIndex(...)->mNonSimProps 读取效果及 sink。
因此此样本是运输条目的效果配置，不存在可从本记录直接加载的车辆模型。
不能据此断言缺包，也不能承诺每一个 C600 记录都有模型。

当前分类依据仅是 group 低位 C600（crates/sc-properties/src/semantic.rs:143）。
当前卡片 models 只查本记录 Vehicle Models，空则扫 RW4 key
（src-tauri/src/package_service.rs:1860）。对于真正有间接模型的其他记录，
Parent / graphics wrapper 是需要逐条验证的解析候选；本样本无此证据。

## 2. 所谓站点资源：截图 F8668FB0

00B1B104:61F0E800:F8668FB0 实际有 50 个场景演员外观条目。
关键反编译来源：
D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/SC/cVignetteGame/FillEntryFromProps.c
函数地址 0x70A7F0，原始 PDB 恢复类型 cVignetteEntryInfo / cVignetteActorEntryInfo。
指定目录 SimCity.c 仅有反编译失败的 #error，不能作为完整源码使用。

| property | 引擎用途 | 源码行 |
| --- | --- | --- |
| 0D9BDA82 | paletteIDs，按 instance 索引 mActorEntries，演员标识 | 598, 624-627 |
| 0D9BDA83 / 84 | skins / skinsMax，mSkin / mSkinMax | 606-608, 656-659 |
| 0D9BDA87 / 88 | outfits / outfitsMax，mOutfit / mOutfitMax | 610-613, 660-663 |
| 0D9BDA86 | bodiesIndex，mBodyIndex | 604, 675-676 |
| 0D2E8A61 | modelNames，mVignetteModelKeys | 601, 677-695 |
| 0DB5452F | simTemplates，GetInstancedModelId | 600, 664-674 |
| 0D6F3BE1 | modelScales，mModelScale，逐演员数组 | 455, 602, 697-698 |
| 0D560650 | speeds，mVignetteSpeed，逐演员数组 | 457, 486-487 |
| 0D6DBF4A | mSelfDestruct | 200-203 |

截图前三个 8017FCE5 / 8017FCE6 / 8017FCE7 是演员标识，而非独立文件 TGI。
本包模型列前 3 行：66BE4E6E、A4EB8B32、A4EB8B32（group 40E02D00，type 0）。
模板列前 3 行：ACCCD1D7、ACCCD1D6、ACCCD1D6（00B1B104:40E02D00）。
这给出了可继续加载模型的真实入口；这些 key 的 type=0 不代表资源不存在。
本样本模型列、模板列、外观列、速度和缩放列均有 50 行。

当前 resolve_names 仅查 registry 的 instance 名称；这些局部演员 ID 不应依赖全局名称库。
当前 UI 四个参数的含义、站点装载说明、floatA/B 标量展示、enabled 展示均应纠正。
-1 在当前 int_column 中被统一抹成 None；引擎此处直接写入外观字段，
并没有把该值解释成“未装载资源”。应保留原值，另行核对 palette 消费端的范围语义。

建议新预览模块使用演员 ID 关联各平行表，展示模型、人物模板、身体索引、
肤色与服装范围、速度、缩放、行为／动画引用；旧 kind 可保留作接口兼容。
不能将所有 E800 的用途仅凭 group 一概定义；本次确定的是上述特征列及截图样本。

## 验证与范围

新增只读诊断 example：crates/sc-properties/examples/preview_probe.rs。
运行 cargo run -p sc-properties --example preview_probe -- <EP1 package> e94b4027 f8668fb0。
成功解析两条真实记录；编译仅有已有库 warning。
没有比较构建产物、没有部署、没有修改游戏包／存档，也未启动游戏。
本轮是调查结论；尚未修改产品预览或完成真实模型加载验证。
## 实现更新

已新增 crates/sc-properties/src/vignette.rs 独立只读解析器；预览 API 在旧 resource-entry 载荷旁附加 scene，旧字段保留兼容，新界面使用 scene。
按演员 ID 合并演员主表、外观表及即兴演员标识表；缺列保持 None，负数原样保留，非有限浮点不送入 JSON。
前端新增 SceneEntryPreview.vue 与 TransportDescription.vue；中英文文案在 property-semantics.ts。
当前展示模型和人物模板的原始引用，尚未自动沿这些引用加载三维模型；即兴演员的区域／动作细表仍可在原始属性表查看。
真实 F8668FB0 验证：50 位演员、0 个列长度问题，首位演员 skin=-1 / speed=5 / scale=1.35。

## 模型引用链与预览续作

新增 actor_graphics.rs（实例化人物模板的部件范围解析）、actor_model_chain.rs（完整TGI引用链）以及 actor_model_preview.rs（源包发现与模型包注册）。
引擎依据：cGraphicsInstancedModels::GetInstancedModelId 0x787AB0，按 instance+group 取 property；cGraphicsInstanced::FillFromProps 0x7872F0 读取 Vehicle Models / LOD1 或人物部件表。
模型引用 type=0 在这条 property-wrapper 路径中按 Property 类型读取，绝不使用仅 instance 的 prop 查找器。
新增 ActorModelPreview.vue：按需解析、网格读取、多模型切换、完整引用链和组合人物模板说明；也用于无模型载具的 Parent / graphics wrapper 引用。
复用既有 MeshPreview 展示原始几何。未实现组合人物组装、演员动画、皮肤和服装范围应用、场景位置及缩放应用；界面明确提示展示范围。
测试：解析库的合成 Parent→LOD1 与错误group拒绝测试；真实安装包 66BE4E6E 包装→RW4 网格解码，ACCCD1D7 模板服装下限64验证通过。
应用级测试可执行文件启动受 STATUS_ENTRYPOINT_NOT_FOUND 阻塞；独立解析库内同一逻辑测试通过，cargo check 应用编译通过。未进行桌面WebGL实际渲染验收。

## UI 统筹自查与溢出修复

复现：SemanticCard .vehicle-mesh 将包含光照控件+画布的整个 MeshPreview 固定为300px，画布本身280px，导致外层卡片底边386而画布底边398。
修复：去掉外层固定高度，由画布自身的16:9比例及180–320px边界决定高度；预览根节点自然增高，range控件列可收缩，动态插入canvas采用deep样式约束。
同时统一新增演员/运输说明的字号、段落间距、主题token与窄列换行；模型按钮增加换行、hover和选中态；引用链默认折叠，保留完整内容。
浏览器检查（真实Vue组件+模拟网格及解析响应，非游戏运行时截图）：中英文 × 280/360/600px视口，共6组，页面/两类卡片无横向溢出，画布位于卡片内部。已人工查看深色主题截图。
截图与复现页位于tmp/ui-review/。vue-tsc、定向ESLint和11项前端测试通过。

## 人物组装、原始动画、调色板与头部坐标修复

新增 actor_assets.rs 读取原始身体/头部、骨架、动画及 sourceTex；SHORT2/FLOAT2 的双权重只在新增人物适配器中补齐四通道，既有导出器不变。若原始身体有动画但导出后不能保留蒙皮则 fail。
真实资源测试：E5537FAB / A4EB8B32 / FAC9B16D / 66BE4E6E 均有骨架、头部挂点与12–13段动画；导出GLB同时具有 skins 和 animations。
两张调色板位于 App 包：2F4E681B:0:69C94C5D（body）与2F4E681B:0:14C5F13F（clothes），均为256×32。App 渲染资源（group 40212001/40212011，instance 0..3）在偏移74068/74092成对引用。source.R=AO / G=paletteV / A=透明度；衣服调色板 alpha 决定肤色与服装混合。按列中心和V线性采样应用外观索引。
新增开关默认关闭：不开启时不请求人物视觉载荷；开启才选择模板范围、读取头身/调色板并循环播放原始动画；关闭销毁视口、纹理及RAF。可重新采样范围内的外观，并切换原始动画片段。片段序号不冒充场景动作名。
浏览器真实导出载荷检查：默认payload请求0次；启用后1次；两帧图像不同；关闭后stage=0且没有新增请求。日夜主题×320/600px布局无溢出。

### 头部歪斜缺陷（用户截图）

错误：直接 joint.add(head) 让头部继承了骨骼局部坐标轴的旋转。
唯一修复依据：SC/cVignettePool/UpdateVignettePool.c，0x7DEA60，668–720行。GetBoneTransforms 与 GetPoseTransforms 只计算头部挂接点；头部 SetRotation 使用演员整体旋转，并非骨骼旋转。
修复为头部与身体共享演员坐标系，每帧把 animated joint world point 经 inverse(bodyWorld) 转回身体 Z-up 局部空间，只更新头部位置；ThreeViewer 的 -90°X 转换在共同祖先统一生效，不额外补固定角度。
actorHeadAttachment.test.ts覆盖共享轴向转换、演员缩放与朝向、骨骼局部90°旋转、动画挂点移动。修复后真实截图头部直立，保存在tmp/ui-review/actor-native-frame-1.png。
未修改冻结property-editor目录，未部署或启动游戏。
