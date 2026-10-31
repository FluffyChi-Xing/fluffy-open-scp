# Prop 偶发退化成标记锥：排查记录（2026-11-01）

## 现象

同一栋建筑有时 prop 正常出模型，有时整批变成红/黄标记锥；截图里大量
红色 prop 锥、青色 path 球，而建筑本体正常。

## 已知链路

```
unit.resourceId → resolve_prop_models（后端脚本资源表/模型 key）
  → 前端 readLotModelMeshes + parseLotModelContainer
  → propModels Map<resourceId, payload>
  → planUnit：propModels.has(resourceId) ? 真模型 : 标记锥
```

任意一步失败都会**静默**退标记锥（设计如此，避免单 prop 失败拖垮整 lot）。

## 本次补强

`usePropertyEditorSession.loadPropModels` 原先只取 `resolution.models[0]`：

- LOD0 文件读取失败、包未就绪、或 LOD0 payload 无 GLB 时，直接放弃该
  resourceId；
- 之后每次 rebuild 都复用同一个空结果 → 该 prop 永远标记锥；
- 不同会话/加载顺序会表现成“偶发”。

现在逐个候选 LOD 尝试，直到拿到非空 GLB；仍失败则保留旧 payload（如果有），
并在 DEV 下打印：

```
[prop-models] unresolved resource ids: 0x........, ...
```

## 仍需确认

如果日志显示某些 resourceId 长期 unresolved，说明问题在后端
`resolve_prop_models`/脚本资源表/包注册，而不是渲染层；需要用户提供该
building 的 TGI 或控制台日志中的 unresolved 列表。

如果 unresolved 为空但画面仍偶发标记锥，则更可能是 rebuild/stale token
时序（加载完成前的一次重建），可再加“加载完成后再排一次 rebuild”兜底；
当前先不引入额外重建，避免放大卡顿。
