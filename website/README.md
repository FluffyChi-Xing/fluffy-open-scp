# OpenSCP Website

官网运维分支：`dev/docs`；源码位置：仓库根目录下的 `website/`。在此目录运行 npm 命令，与根目录桌面应用的构建相互独立。

Vercel 部署时选择 `dev/docs` 作为 Production Branch，Root Directory 设置为 `website`；本目录的 `vercel.json` 已配置构建命令、静态输出与 clean URLs。正式上线前设置 `SITE_URL`，Google/Bing 验证值可稍后补充。

```powershell
cd website
npm ci
npm run dev
```

独立的 OpenSCP 中英文官网：VitePress + Vue 3 + Tailwind CSS 4 + 原生 WebGL shader。所有运行资源均在本项目中，无需原应用目录、外部字体 CDN 或 shader 服务。

## 本地运行

需要 Node.js 22 或更新版本。

```powershell
npm ci
npm run dev
```

访问终端显示的本地地址。生产构建与预览：

```powershell
npm run check
npm run build
npm run preview
```

静态输出位于 `docs/.vitepress/dist`。部署到支持干净 URL 的静态托管服务（将 `/about` 解析为 `/about.html`，并提供 `404.html`）。

## 迁移或导出官网源码

将本目录的源文件整体复制到目标目录，然后执行 `npm ci` 和 `npm run build`。无需复制 `node_modules`、`.context`、`test-results`、`artifacts` 或 `.vitepress/dist`、`.vitepress/cache`。

也可以运行 `./scripts/export.ps1`，生成仅含源码、锁文件与本地素材的 `artifacts/openscp-website-时间戳.zip`。解压后即可作为独立项目运行。

源文件放在哪个 Git 文件夹与站点 URL 无必然联系。若部署到 URL 子路径，例如 `/openscp/`，构建前设置：

```powershell
$env:SITE_BASE = '/openscp/'
npm run build
```

根域名部署使用默认 `/`。环境变量路径必须以 `/` 开头和结尾。请勿把桌面应用的 package.json 与本项目合并。

## SEO 与 Vercel 部署配置

每页生成双语标题、description、Open Graph 与 Twitter 分享元数据。设置正式域名后，构建会生成 canonical、中英文 hreflang、WebPage JSON-LD、sitemap.xml 与 robots.txt。

在 Vercel 项目环境变量中设置以下值（更改后重新部署）：

| 环境变量 | 用途 |
| --- | --- |
| `SITE_URL` | 正式站点 origin，例如 `https://你的域名`，不要包含子路径 |
| `SITE_BASE` | URL 子路径，默认 `/`；放进 Git 子文件夹不意味着必须设置 URL 子路径 |
| `GOOGLE_SITE_VERIFICATION` | Google Search Console 验证 meta 的 content 值，可暂不设置 |
| `BING_SITE_VERIFICATION` | Bing Webmaster Tools 验证 meta 的 content 值，可暂不设置 |

未设置 SITE_URL 时，不生成假域名 canonical 或 sitemap，并输出 noindex；Vercel Preview 环境也默认禁止索引。正式部署必须设置 SITE_URL 才开放索引。Google/Bing 的验证值留空时不输出验证标签。

目前不加载 Google Analytics、Microsoft Clarity 或其他追踪脚本。后续提供具体脚本类型和 ID 后，可在 `docs/.vitepress/config.ts` / `seo.ts` 集中加入；验证 meta token 与统计脚本 ID 不能混用。Vercel 环境变量由构建读取；本地测试请先在 PowerShell 中设置 `$env:SITE_URL` 等变量。

部署时将官网文件夹设为 Vercel Root Directory，Build Command 为 `npm run build`，Output Directory 为 `docs/.vitepress/dist`。使用干净 URL 时启用 `cleanUrls`。若站点部署在子路径，需让域名根部 `/robots.txt` 指向对应 sitemap，或直接向搜索平台提交 sitemap。

## 内容与双语维护

- 中文默认路径：`docs/*.md`、`docs/guide/`、`docs/research/`。
- 英文镜像路径：`docs/en/`，文件名与目录结构必须对应。
- 页面文字与无障碍标签：`docs/.vitepress/theme/messages.ts`。
- 切换语言保留对应页面；`useLocale.ts` 集中处理路由与部署 base。
- 首页组件：HeroSection、WorkbenchSection、ResearchSection、ProgressSection。
- 文章由 VitePress Markdown 渲染；DocPage 负责阅读布局、导航和目录。
- 所有新增界面文字都需同时维护 zh-CN 与 en-US。功能截图按 zh-CN / en-US 成对保存，随页面语言切换。

### 发布研究文章

1. 在 `docs/research/` 和 `docs/en/research/` 创建同名 Markdown。
2. 填写 frontmatter：`title`、`description`、`research: true`。
3. 在 `messages.ts` 两种语言的 `articles` 中登记 slug、分类、标题、摘要及依据说明。
4. 文章分别写明依据、观察、推论与未知项；给出源码位置、样本范围或可复核记录。不要把规划写成已完成，也不要将摘要描述为本次重新验证的结论。
5. 运行构建与浏览器检查，确认双语页面及链接可达。

研究首批内容是已有研究的双语摘要（GlassBox、贴花、资源格式），使用固定 Git commit 引用以减少 master 重整导致的链接变化。完整材料仍以原仓库研究记录为准。内容快照为 2026-10-15，不是实时开发看板。

## 夜间模式

导航栏提供中英文标注的夜间模式开关，使用 VitePress 内置主题状态。首次访问跟随系统，手动选择保存至浏览器；语言切换、页面跳转和刷新保留选择。首页 shader 与正文同步切换，减少动态效果时也会即时重绘静态帧。

## Shader 与性能

`composables/terrain.ts` 使用原生 GLSL 绘制缓慢流动的冰蓝、天空蓝与钴蓝色场。像素比限制为 1.5，绘制约 25 fps；离开视口或切到后台时暂停，减少动态效果偏好下显示静态帧。WebGL 不可用时保留 CSS 背景，不影响正文。组件卸载时释放 GPU 资源及观察器。

## 验证

```powershell
npx playwright install chromium
npm run build
npm test
```

浏览器测试涵盖双语导航、文章直达、手机菜单、键盘标签页、横向溢出、页面链接和截图。测试自动启动生产预览。

若已安装 Chrome，可设置 `$env:PW_CHANNEL = 'chrome'` 后运行 `npm test`，无需另行下载测试浏览器。

## 素材来源

`docs/public/images/logo.png` 原样复制自 OpenSCP 的 `docs/assets/scp-logo.png`；新版截图来自用户提供的 `D:/rust/packages/openscp-assets/`，原样复制到 `docs/public/images/zh-CN/` 与 `en-US/`；包含资源总览、源文件解析、模型预览、地图和 Raster 五组。素材映射见 `docs/public/images/SOURCES.md`，运行时无需原素材目录。字体 Manrope 通过 npm 本地打包。SimCity 及相关游戏资产的权利归各自权利人，OpenSCP 为独立社区项目。

构建工具使用 Vite 6.4.4 或更新的 6.x 修复版本覆盖 VitePress 1.x 的旧间接依赖；升级 VitePress 后应重新评估此 override。
