---
title: 开始使用
description: 安装 OpenSCP，配置游戏目录并打开第一个资源包。
---
# 从第一个 .package 开始

本指南依据当前仓库 README 整理。你需要本机的 SimCity (2013) 游戏文件；OpenSCP 不附带游戏资源。

## 安装与启动

1. 打开 [GitHub 发布页](https://github.com/FluffyChi-Xing/fluffy-open-scp/releases)，查看可用版本和对应的安装说明。
2. Windows 用户可按发布资产选择 MSI 或 NSIS 安装程序。其他系统的支持情况请以具体发布说明为准。
3. 启动 OpenSCP。在 **设置 → 游戏目录** 中选择包含 `SimCityData` 的安装目录，例如 `D:\ea-games\SimCity`。
4. 等待资源包扫描完成，再从左侧资源树选择一个 `.package`。

也可以在包列表中直接双击 `.package`，无需先配置游戏目录。

## 找到一个资源

资源页支持按 **TGI（Type / Group / Instance）** 筛选、搜索与分页。选择资源后，工作台会根据类型打开模型、贴图、属性表、文本、音视频或十六进制预览。

第一次探索可以先查看贴图与属性表，再打开 RW4 模型。跨包引用的材质或贴花可能需要完整游戏目录才能解析。

## 导出与编辑

- 模型可导出为 glTF 2.0（`.glb`）或 Wavefront `.obj`。
- 贴图可导出为 PNG / JPG / TGA / DDS；属性表支持 JSON / TXT。
- Locale 文本编辑可导出 overlay package，用相同 TGI 覆盖原始文本资源。

编辑后先检查导出结果，再进入游戏验证。建议保留原始游戏包与工作副本，以便比较修改前后的表现。

## 音视频所需工具

音视频转码依赖外部工具，安装包不会自动附带。其他资源浏览功能不需要这些工具。

| 工具 | 用途 | 获取方式 |
| --- | --- | --- |
| vgmstream | Wwise 音频解码为 WAV | [项目主页](https://github.com/vgmstream/vgmstream) |
| FFmpeg | VP6 视频转换为 MP4 | [官方网站](https://ffmpeg.org) |

将工具加入系统 `PATH`，或按应用约定放在 `Tools/vgmstream/` 与 `Tools/ffmpeg/` 中。未安装时，应用会提示缺少的工具。

## 下一步

阅读[工作台使用指南](./workbench)，了解资源浏览、地图预览、Raster 绘制与 Mod Studio 的区别；在[开发进度](../development)中确认当前能力边界。
