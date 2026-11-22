export const panelRuntimeZh = {
  required: '必需的面板运行时依赖',
  instruction: '此依赖包必须与建筑包一起加载。仅部署建筑包时，自定义悬停和点击弹窗无法初始化。',
  installation: '在游戏关闭后按测试包归档流程安装，并在一次性测试城市中验证。多个使用相同运行时的建筑共用一份。',
};

export const panelRuntimeEn = {
  required: 'Required panel runtime dependency',
  instruction: 'Load this dependency alongside the building package. Deploying only the building package prevents custom hover and click panels from initializing.',
  installation: 'Install through the test-package archive workflow with the game closed, then verify in a disposable test city. Buildings using the same runtime share one copy.',
} satisfies Record<keyof typeof panelRuntimeZh, string>;
