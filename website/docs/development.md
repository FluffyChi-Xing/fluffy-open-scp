---
title: 开发进度
description: 已交付能力、正在完善的编辑器与未来 Modding Suite 规划。
---
# 把进展摊开，也把边界说清

**内容快照：2026-10-15。** 本页依据项目 README 与路线图整理，不是实时 CI 看板，也不代表某个功能已包含在所有发行版中。查看发布说明以确认版本能力。

## 已可用：资源工作台与写回底座

- DBPF / RefPack、RW4、属性表解析，及模型、贴图与文本预览。
- TGI 复写检测：查看原版与多个 mod 的覆盖关系。
- Locale 编辑与确定性 overlay package 导出。
- 路线图执行记录已标记完成高度图回写链路库化、debug 工具 overlay 产品命令，以及蓝图面板基础验证。

“可用”说明已有实现或交付记录，不等于所有格式变体均已覆盖。

## 正在完善：编辑器与预览

Property 编辑器、游戏 UI 编辑装配管线，以及模型视口的材质、日夜和供电状态表现，是 README 记录的持续开发方向。

游戏 UI 预览与完整可编辑 UI 工作流是不同阶段。项目截图中的界面入口不应自动理解为全部功能已完成。

## 规划中：声明式 Modding Suite

围绕 `openscp.mod.toml` 描述项目，并逐步补齐：

- OBJ 导入与 LOD 管理的完整工作流；
- 资源依赖解析与自动校验；
- 确定性 overlay 构建与可行动诊断；
- 预览、文件监听，以及后续高级材质和骨骼编辑。

这里列出的是方向，不是承诺交付日期。

## 如何追踪变化

查看 [Releases](https://github.com/FluffyChi-Xing/fluffy-open-scp/releases) 获取实际发布内容，通过 [Issues](https://github.com/FluffyChi-Xing/fluffy-open-scp/issues) 了解公开问题与反馈。

本页依据：[README](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/README.md)、[开发优先级](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/docs/roadmap/priorities.md)、[Modding Suite 规划](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/docs/roadmap/modding-suite.md)。这些路径对应当前源码结构，后续分支整理可能调整。
