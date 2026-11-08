# SC5 Asset Packager

窗口版资产打包器：可以新建或打开项目，导入 GLB、LOTM、地块 Package、PNG 预览图和 JSON 配置，浏览输入文件，设置游戏分类、名称、版本、造价与维护费，然后生成检查报告、编译计划或导出 Package。

软件只写入当前项目目录的 `input`、`reports`、`build` 和 `output`，不会写入游戏安装目录。后续连接外部编译服务时按一次请求一次返回的 HTTP 接口设计，当前版本不使用 WebSocket 长连接。

## 快速开始

```powershell
python sc5_packager.py init .\我的资产 --name "Dalian Hope Tower" --category education
python sc5_packager.py inspect .\我的资产
python sc5_packager.py build .\我的资产 --dry-run
```

项目目录约定：

```text
我的资产/
├─ project.json
├─ input/       # GLB、LOTM、lot.package、native.json 等
├─ reports/
├─ build/
└─ output/
```

`inspect` 只读检查输入文件、哈希、导出元数据和基本图片信息，不会访问或修改游戏目录。

0.3.0 已接通内置 LOTM9 编译器。打开资产目录，选择教育或公园分类、费用和游戏资源目录，点击导出即可生成离线候选 Package。详见 `release/0.3.0/使用说明.md`。教育分类只控制菜单位置，不提供学位/校车服务。

`build --dry-run` 只生成编译计划。不带 `--dry-run` 时，未配置外部后端的项目使用内置编译器。`gameDirectory` 可设置包含 SimCityData 的游戏根目录，只读。推荐 `compact_2048` 表面图集方案。

后端命令支持 `{project}`、`{output}`、`{config}` 三个占位符。命令的输出会保存到 `reports/backend.log`，返回码会保存到 `reports/build-result.json`。

`backend_native.py` 是当前研究流水线适配器：它调用指定的研究编译脚本，寻找最新 Package 并复制到项目 `output/`。现阶段它适合验证已有资产流水线；通用 GLB/LOTM 编译器完成后，只需替换 `--pipeline`。


## 0.5.0 菜单位置

窗口版现支持 22 个中文菜单位置，包括教育、公园、电力、供水、污水、垃圾处理、消防、医疗、警察、政府、公共交通子菜单、文化地标和部分城市专精菜单。选择菜单位置只调整入口，保留既有展示资产运行配置。新增位置通过离线核验，待游戏测试。程序及说明位于 `release/0.5.0`，完整研究依据见 `阶段报告_0.5.0_菜单位置扩展.md`。
