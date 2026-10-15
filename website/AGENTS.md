# OpenSCP website

- 独立 VitePress 网站。不要引入对兄弟目录或原桌面应用的运行时依赖。
- Vue 使用 Composition API、script setup 与 TypeScript。
- 所有界面文字（含 aria 标签、空状态、404、按钮）同时维护 zh-CN / en-US。
- 中英文 Markdown 路由一一对应；内部链接和图片兼容 SITE_BASE 子路径部署。
- 研究区分观察、推论和未知；开发进度区分可用、进行中与规划。不要编造版本、发布日期或测量结果。
- 修改后运行 npm run check、npm run build；路由或交互变更运行 npm test。
- Shader 保留 SSR 安全、静态降级、减少动态效果、后台暂停和清理逻辑。
