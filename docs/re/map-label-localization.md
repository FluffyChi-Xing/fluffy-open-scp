# 地图标签语言切换

城市名原先仅从中文语言包读取到 `name`，Three.js canvas 标签生成后也未订阅语言切换。
现在地块同时携带 `name` 和 `nameEn`，按 UI 语言选择；伟大工程通用标签使用 i18n。
切换语言只替换文字纹理及其尺寸，释放旧纹理，保留相机、地形与图层状态。
高度标尺、复位按钮及尺寸提示同步国际化。

## 英文名称来源

`region_names_en.json` 是 Game 包区域模板注册表引用的 190 条区域/城市名称，
按原始 string ID 索引，避免将模板内部代号当作零售英文名。
来自本地 `simcity_offline/SimCity：Cites of Tomorrow/SimCityData/Locale` 的语言资源。
该安装的目录内容实际颠倒：`zh-tw` 包含英文，`en-us` 包含中文；导出时核对文本内容。
主安装只有中文语言包，因此保存精简名称表作为英文回退，不依赖开发者的绝对路径。

可使用 `locale_names_probe <Game.package> <output.json> <locale.package> [...]` 重建，
仅提取注册表引用的名称，不打包地图纹理或整套语言资源。
未知城市缺少英文时使用本地化的 City Site + UID，避免英文 UI 又显示中文标签。

## 验证

三一岬的三个城市按同一 string ID 匹配为 Trinity Point、Clearwater、Norwich Hills；
伟大工程显示 Great Work Site，区域名为 Cape Trinity。
真实 WebGL 已验证英文→中文→英文即时更新，无控制台错误。
Rust 回归检查字符串 ID 和 `nameEn` 序列化字段，前端执行类型与 i18n 检查。
