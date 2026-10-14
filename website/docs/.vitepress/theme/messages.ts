export const messages = {
  zh: {
    discord: '加入 Discord 社群',
    darkMode: '夜间模式', lightModeAction: '切换为日间模式', darkModeAction: '切换为夜间模式',
    nav: ['项目介绍', '使用指南', '开发进度', '研究档案'], menu: '导航菜单', close: '关闭菜单', skip: '跳到正文', language: 'Switch to English', home: '返回首页',
    tag: '为 SimCity (2013) 而造 · 开源模组工具链', title: '打开资源，', titleAccent: '看见城市的另一面。',
    intro: '从一个 .package 文件出发，探索模型、材质与模拟系统。让游戏资源变得可读，让你的创作有迹可循。', start: '开始探索', source: '查看源码', release: '查看发布版本',
    heroNote: 'Rust 驱动 · 本地运行 · MIT 开源', visualTitle: '资源工作台', visualCaption: '真实应用截图 · Property 编辑器', visualTag: '从资源到可见世界',
    strip: ['DBPF 资源解析', 'RW4 模型预览', 'Overlay 模组写回', 'GlassBox 研究'],
    workEyebrow: '探索 / 编辑 / 构建', workTitle: '一座城市，从每个资源开始。', workIntro: '把分散的文件、属性和渲染线索，放回同一个工作流。', screenshotNote: '真实应用截图 · 随网站语言切换中英文界面。',
    tabs: ['资源浏览', '模型与材质', '地图工作台', 'Raster 绘制', '资源总览'],
    tabTitles: ['读懂每一个 .package', '让模型回到真实的材质里', '从区域全景，深入地形数据', '把地表细节变成可编辑的画布', '先看全貌，再深入每个资源'],
    tabDescriptions: ['按 TGI 筛选与定位资源，预览文本、贴图和属性，再导出到熟悉的工具。', '查看 RW4 模型、LOD 与材质通道，在日夜与供电状态下观察建筑表现。', '查看区域地形合成、城市地块与资源图层，为地图编辑建立空间上下文。', '使用四通道 LotMask 画布，绘制地面与停车位标线，并保存 overlay package。', '查看已扫描资源包的数量、资源类型与容量分布，从总览找到下一步探索的方向。'],
    guideLink: '查看操作指南',
    researchEyebrow: '开放研究档案', researchTitle: '理解它，才能重新创造它。', researchIntro: '记录从二进制到屏幕的推理过程。每一篇都说明依据、已知结论与仍待验证的部分。', allResearch: '浏览研究档案', read: '阅读研究',
    articles: [
      { slug: 'glassbox', category: '引擎架构', title: 'GlassBox：城市如何运转', description: '子系统注册、三阶段更新，以及模拟与渲染之间的关系。', meta: '架构笔记 · 依据已归档' },
      { slug: 'decals', category: '渲染管线', title: '一张贴花，如何抵达建筑表面', description: '从 Property 字典与纹理载体，到投影盒和材质通道。', meta: '渲染笔记 · 依据已归档' },
      { slug: 'formats', category: '资源格式', title: '从 DBPF 开始读懂资源', description: '理解 TGI 索引、RefPack 压缩与 RW4 的职责边界。', meta: '格式入门 · 依据已归档' }
    ],
    progressEyebrow: '持续构建中', progressTitle: '把进展摊开，也把边界说清。', progressIntro: '以仓库文档为依据，区分当前能力与未来计划。', roadmapLink: '查看完整开发进度',
    phases: [ { state: '已可用', title: '资源工作台与基础写回', body: '资源浏览、模型预览、TGI 复写检测、Locale 编辑与 overlay 导出。' }, { state: '持续完善', title: '编辑器与预览体验', body: 'Property 编辑、游戏 UI 编辑及材质表现，逐步补齐验证与工作流。' }, { state: '规划中', title: '声明式 Modding Suite', body: '项目描述、LOD 与依赖管理、自动校验，以及确定性构建。' } ],
    ctaTitle: '下一次探索，从打开一个包开始。', ctaBody: '为好奇的玩家、模组作者与引擎研究者而构建。',
    footer: '让城市的内部世界，成为开放的知识。', legal: '独立社区项目，与 EA / Maxis 无隶属关系。SimCity 商标归各权利人所有。', onPage: '本页内容', docsTitle: '探索 OpenSCP', updated: '内容快照：2026-10-15', notFound: '这条路径还没有被探索。', notFoundBody: '页面不存在或已移动，请返回首页继续浏览。', articleBack: '所有研究',
  },
  en: {
    discord: 'Join our Discord',
    darkMode: 'Dark mode', lightModeAction: 'Switch to light mode', darkModeAction: 'Switch to dark mode',
    nav: ['About', 'User guide', 'Development', 'Research'], menu: 'Navigation menu', close: 'Close menu', skip: 'Skip to content', language: '切换为简体中文', home: 'Back to home',
    tag: 'Built for SimCity (2013) · Open-source modding tools', title: 'Open the assets.', titleAccent: 'See another side of the city.',
    intro: 'Start with a .package file. Explore models, materials and simulation systems. Make game assets readable, and your next creation possible.', start: 'Start exploring', source: 'View source', release: 'View releases',
    heroNote: 'Powered by Rust · Runs locally · MIT licensed', visualTitle: 'Asset workbench', visualCaption: 'Application capture · Property editor', visualTag: 'From assets to a visible world',
    strip: ['DBPF parsing', 'RW4 model preview', 'Overlay mod exports', 'GlassBox research'],
    workEyebrow: 'EXPLORE / EDIT / BUILD', workTitle: 'A city starts with its assets.', workIntro: 'Bring scattered files, properties and rendering clues into one workflow.', screenshotNote: 'Actual application captures, matched to your selected language.',
    tabs: ['Asset browser', 'Models & materials', 'Map workbench', 'Raster painting', 'Resource overview'],
    tabTitles: ['Make sense of every .package', 'See models with their materials', 'From the region to its terrain data', 'An editable canvas for ground detail', 'See the whole collection before diving in'],
    tabDescriptions: ['Find assets by TGI, preview text, textures and properties, then export to the tools you know.', 'Inspect RW4 models, LODs and material channels across time-of-day and power states.', 'Explore regional terrain, city lots and resource layers to give map editing spatial context.', 'Paint four-channel LotMasks and parking markings, then save an overlay package.', 'Review scanned packages, resource types and size distributions to decide where to explore next.'], guideLink: 'Read the user guide',
    researchEyebrow: 'OPEN RESEARCH ARCHIVE', researchTitle: 'Understand it. Then reimagine it.', researchIntro: 'Follow the reasoning from binary data to the screen. Each note separates evidence, established findings and open questions.', allResearch: 'Explore the archive', read: 'Read research',
    articles: [
      { slug: 'glassbox', category: 'ENGINE ARCHITECTURE', title: 'GlassBox: how the city runs', description: 'Subsystem registration, three-stage updates, and the relationship between simulation and rendering.', meta: 'Architecture notes · Sources included' },
      { slug: 'decals', category: 'RENDERING PIPELINE', title: 'How a decal reaches a building', description: 'From Property dictionaries and texture containers to projection volumes and material channels.', meta: 'Rendering notes · Sources included' },
      { slug: 'formats', category: 'ASSET FORMATS', title: 'Reading assets, starting with DBPF', description: 'TGI indices, RefPack compression and the boundary between containers and RW4 payloads.', meta: 'Format primer · Sources included' }
    ],
    progressEyebrow: 'BUILDING IN THE OPEN', progressTitle: 'Visible progress. Honest boundaries.', progressIntro: 'A documentation-based view of what works today and what comes next.', roadmapLink: 'View development progress',
    phases: [ { state: 'Available', title: 'Asset workbench & overlay exports', body: 'Asset browsing, model previews, TGI override detection, Locale editing and overlay exports.' }, { state: 'In progress', title: 'Editors & preview fidelity', body: 'Property and game UI editing, material rendering, validation and connected workflows.' }, { state: 'Planned', title: 'Declarative Modding Suite', body: 'Project manifests, LOD and dependency management, automatic checks and deterministic builds.' } ],
    ctaTitle: 'Your next discovery starts with a package.', ctaBody: 'Built for curious players, mod authors and engine researchers.', footer: 'Opening the inner world of the city to everyone.', legal: 'An independent community project, unaffiliated with EA / Maxis. SimCity trademarks belong to their respective owners.', onPage: 'On this page', docsTitle: 'Explore OpenSCP', updated: 'Content snapshot: 2026-10-15', notFound: 'This path is still unexplored.', notFoundBody: 'This page does not exist or has moved. Return home to keep exploring.', articleBack: 'All research',
  }
}
