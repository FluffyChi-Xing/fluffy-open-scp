export const panelRuntimeZh = {
  required: '需要共享 Uni Panel 运行库',
  instruction: '每套游戏安装只加载一份 Uni Panel，供所有自定义建筑面板共用。',
  installation: '导出时请选择实际启动游戏的 SimCityData。游戏关闭后按归档台账流程替换共享运行库。',
  export: '导出共享 Uni Panel',
  selectGameData: '选择实际启动游戏的 SimCityData',
  selectExportFolder: '选择游戏目录之外的导出文件夹',
  exportFailed: '无法导出共享 Uni Panel。{detail}',
};

export const panelRuntimeEn = {
  required: 'Shared Uni Panel runtime required',
  instruction: 'Each game installation loads one Uni Panel runtime for all custom building panels.',
  installation: 'Select the SimCityData folder used to launch the game. Replace the shared runtime through the archive workflow with the game closed.',
  export: 'Export shared Uni Panel',
  selectGameData: 'Select the SimCityData folder used to launch the game',
  selectExportFolder: 'Select an export folder outside the game installation',
  exportFailed: 'Could not export shared Uni Panel. {detail}',
} satisfies Record<keyof typeof panelRuntimeZh, string>;
