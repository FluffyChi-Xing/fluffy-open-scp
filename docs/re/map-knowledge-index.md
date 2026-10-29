# 地图开发知识账本

本文是地图开发逆向资料的入口。各主题文档把结论分成三类：

- **已实锤**：有静态包、EGB、反编译代码或可重复测试支持。
- **高置信**：多份样本吻合，但还缺少游戏内双向对拍。
- **待验证**：只能作为实现假设，不能用于生成可交付 mod。

## 当前主题

| 主题 | 文档 | 当前状态 |
|---|---|---|
| 静态地图基线与全图校验 | [map-static-bootstrap.md](map-static-bootstrap.md) | 15 区域/130 城静态路网与六类资源接入；无需存档对齐 |
| 道路网络、PathEntry 与桥梁外观 | [map-road-network-facts.md](map-road-network-facts.md) | 曲线、外观表、模型链已接入预览；挡土墙/隧道姿态仍待对拍 |
| 资源图层、ED 与 brush | [map-resource-layer-facts.md](map-resource-layer-facts.md) | EP1 原始 typed map 已按城市 UID/environment 绑定，地下水包含区域 ED |
| 新区域初始化与 EGB 生命周期 | [map-bootstrap-egb-facts.md](map-bootstrap-egb-facts.md) | 已找到零售静态 bootstrap；独立生成可被游戏接受的新 EGB 仍未完成 |
| 坐标、UnitTransform 与渲染故障 | [map-coordinate-rendering-facts.md](map-coordinate-rendering-facts.md) | 坐标边界已统一；交叉口和 sweep 模型仍需实机对拍 |
| 摄像机边界与区域编辑相机 | [map-consumers-ui-bridge-camera.md](map-consumers-ui-bridge-camera.md) | 已确认 `regionedit` 只切换 controller；主 bounds、relative-move 二次限制和地面约束已拆开，零售运行时改写仍待验证 |
| 编辑器目标与 package 产物 | [map-editor-flow-requirements.md](map-editor-flow-requirements.md) | 需求账本；不是已实现功能清单 |
| 地图实际消费者、UI 通信与相机边界 | [map-consumers-ui-bridge-camera.md](map-consumers-ui-bridge-camera.md) | 区分模板计数/运行时 boxes/registry 文案；定位原生回调和开发版多层相机限制，零售动态验证待完成 |
| 当前离线发售版命令/事件协议 | [retail-ui-command-event-protocol.md](retail-ui-command-event-protocol.md) | 核对实际 EXE 与安装包 JS；默认原生回调和直接事件推送；隔离合约测试通过，尚无实机抓流 |

| 新样本地图编辑器突破 | [map-editor-pdb-breakthrough-20261030.md](map-editor-pdb-breakthrough-20261030.md) | PDB/source-tree 已拆出 PlayArea、三类坐标、地图资源闭包和 Terrain Editor 调试会话；运行时验收矩阵待执行 |

| 零售 EAWebKit 运行时 | [retail-eawebkit-runtime.md](retail-eawebkit-runtime.md) | 确认 JSC；实机读出两个页面及 Client 注册表；DebugLog 空处理、Inspector 工厂返回空；已捕获原生调用元数据；参数与事件待解码 |

## 证据等级与文件约定

核心样本只读放在 `tmp/`，包括：

- `tmp/save3-region.json`：CapeTrinity 干净新存档区域 EGB 解码结果；
- `tmp/dump-30360939.log`：静态道路 Unit `30360939` 属性；
- `tmp/dump-road-appearance.log`：28 行道路外观表；
- `tmp/dump-pathentry-017a430a.log`：12 行 PathEntry 静态/实时挤出表；
- `tmp/static-path-match-save3.log`：区域曲线与静态 Unit PathEntry 的匹配结果；
- `tmp/save3-unit-resource.log`：区域单位、路径和属性的摘要。

这些文件是探针产物，不是游戏输入；不能把实验 EGB 直接复制成新的地图存档。

## 已确认的工程边界

1. package 静态资源可以提供地图模板、地形、资源 brush、Unit 属性、PathEntry 和模型材质。
2. 道路中心线和区域单位的原始世界实例可从 EP1 内的 EcoGame bootstrap 读取；ED 道路痕迹不能替代它们。
3. 新区域首先在内存中初始化，EGB 是随后写出的状态快照，不是启动新区域的唯一输入。
4. OpenSCP 预览必须区分静态资源、区域 EGB 参考快照和玩家城市存档，不能把三者混为一个数据源。

| 交通 handler 与车辆规则类型 | [traffic-handler-types-and-police-correlation-20261030.md](traffic-handler-types-and-police-correlation-20261030.md) | 新样本闭合 12 个运行时 handler 注册项；确认警车四类 TransportEntry 共享 `cartraffic`，并给出规则/资源/Agent 调试闭包 |
