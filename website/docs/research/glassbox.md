---
title: GlassBox：城市如何运转
description: 子系统注册、三阶段更新，以及模拟与渲染的关系。
research: true
---
# GlassBox：城市如何运转

> 架构笔记 · 文档摘要 · 整理于 2026-10-15。本文依据 OpenSCP 已归档研究，未在官网开发过程中重新执行逆向验证。

在 SimCity (2013) 中，可见的城市不是单一渲染对象。地形、道路、分区、灾害、音频与模拟系统各自承担职责，再通过运行时管理结构连接起来。

## 从命名空间理解分层

项目对反编译结果的整理，将 `GB` 对应到引擎运行时、`SC` 对应到游戏模拟与相关渲染常量结构，`SP` 对应到平台和渲染支撑。命名空间是理解组织方式的线索，不应简单等同于独立 DLL 边界。

| 层 | 研究中归纳的职责 |
| --- | --- |
| GB | 游戏管理、Eco 场、状态机、同步与 IO |
| SC | Agent、交通、Zoning、Lot、Terrain、Disaster |
| SP | 平台支撑、渲染与 shader 常量缓冲宿主 |

## 子系统如何被找到

归档研究记录了通过 GameManager 的服务定位接口，按标识取得子系统的机制。注册项包括 GraphicsGame、RoadManager、TerrainGame 与 ZoningGame 等。

这一观察意味着，研究某个画面效果时，不能只追踪对应资源文件；还需要确认哪个运行时子系统消费它，以及更新发生在什么阶段。

## 三阶段更新

文档将更新过程归纳为 **Begin → Process → End**。这提供了一个分析时序的骨架，但不能仅凭阶段名称推导出所有线程关系或完整调度顺序。

模拟侧的 E（Eco）与渲染侧的 G（Graphics）通过共享数据和描述符发生联系。归档示例中的 terrain typed maps 由引用计数对象共享持有，而非简单地逐帧完整复制。

## 对模组开发的意义

资源存在，不代表它在当前状态下就会被消费。修改属性后，应同时记录资源引用、子系统状态和游戏内观察结果，避免把预览变化直接当成模拟行为已改变。

## 依据与边界

- 原始研究：`docs/blog/glassbox-engine-architecture.md`。
- 原文依据：反编译记录、真实资源包探针与运行时观察。
- 本文只提取架构概念；不同发行版本的地址、布局与实现细节仍需分别验证。
- [阅读归档原文](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/docs/blog/glassbox-engine-architecture.md)。

继续阅读：[贴花渲染管线](./decals) · [资源格式入门](./formats)
