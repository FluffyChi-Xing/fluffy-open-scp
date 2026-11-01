/**
 * 生成器（spawner 锚点）面板文案：资产编辑器属性页签 + Property Viewer 共用。
 * 术语以引擎事实为准——"生成器"是 lot 作者对 UnitLocation 命名锚点的俗称，
 * 它只表达位置，不表达人数与角色外观。
 */

export const spawnerPanelZh = {
  title: "生成器锚点",
  badge: "生成器",
  intro:
    "锚点 = 命名位置（名字 id + 局部变换）。引擎只用它给演出 / UI / 相机取点，不决定站着谁、有几个人。",
  anchorName: "锚点名字",
  anchorNamePlaceholder: "例如 NorthDoor",
  anchorNameHint: "写入时按 FNV-1（小写折叠）换算成名字 id；引擎侧不可逆。",
  anchorId: "名字 id",
  anchorUnnamed: "未命名",
  anchorUnnamedHint: "未命名的锚点引擎无法按名取点，写入时还会截断后续锚点。",
  useUiAnchor: "设为 UI 气泡锚点",
  useCameraAnchor: "设为镜头锚点",
  clearName: "清除名字",
  commitName: "写入名字",
  resetName: "还原",
  position: "局部位置",
  positionHint: "位置在「坐标」页签用视口手柄调整。",
  anchorKind: {
    ui: "UI 气泡锚点",
    camera: "镜头锚点",
    custom: "自定义名字",
    unnamed: "未命名",
  },
  anchorKindHint: {
    ui: "0x141EED53：引擎硬编码的语音气泡 / UI 特效锚点，镜头缩放也查它。",
    camera: "0x9901C8F9：引擎第二个硬编码位置空间，演出取点走同款分支。",
    custom: "自定义名字：只有同名的演出 / UI 资产才会引用它。",
    unnamed: "未命名：没有任何资产能引用它。",
  },
  issue: {
    "anchor-missing": "未命名锚点：引擎无法按名取点，写入时 ids 列会在此截断。",
    "anchor-duplicate": "名字重复：同名锚点互相覆盖（引擎按名字建表）。",
    "transform-missing": "缺少变换：transforms 列会在此截断。",
    "anchor-invalid": "名字 id 非法（必须是非零的 32 位整数）。",
  },
  duplicateOf: "与 {count} 个锚点同名",
  legacyTitle: "历史列（引擎不消费）",
  legacyHint:
    "count / countRandom / agent 三列曾被误标为「生成数量 / agent 引用」，实测属建筑变体系统，与锚点无关；这里只读展示，写回时不再生成。",
  castTitle: "角色与外观",
  castHint:
    "谁站在这里由 agent 决定：外观来自 transport entry 的外观表（如 GI_Simm_politician_ma = outfit 92 的西装）。锚点本身不携带角色信息，改锚点名不会改变小人外观。",
  listTitle: "本 Lot 的锚点",
  listEmpty: "还没有锚点。从左下预设拖入视口即可放置。",
  listSelected: "已选中",
  listSearch: "搜索锚点名字 / id",
  stats: "{total} 个锚点 · {named} 个已命名 · {unnamed} 个未命名",
  statsDuplicate: " · {count} 个重名",
  presetStandard: "标准锚点",
  presetStandardHint: "只有位置，名字留空待填",
  presetUi: "UI 气泡锚点",
  presetUiHint: "预填 0x141EED53",
  presetCamera: "镜头锚点",
  presetCameraHint: "预填 0x9901C8F9",
  readonlyHint: "只读视图：在资产编辑器中解锁编辑后才会写入。",
  selectAnchor: "选中该锚点",
} as const;

export const spawnerPanelEn = {
  title: "Spawner anchor",
  badge: "Spawner",
  intro:
    "An anchor is a named location (name id + local transform). The engine only uses it to resolve points for vignettes, UI and the camera — never to decide who stands there or how many.",
  anchorName: "Anchor name",
  anchorNamePlaceholder: "e.g. NorthDoor",
  anchorNameHint:
    "Hashed with FNV-1 (lower-cased) on write; the engine side is not reversible.",
  anchorId: "Name id",
  anchorUnnamed: "Unnamed",
  anchorUnnamedHint:
    "An unnamed anchor cannot be looked up by name, and writing it truncates the following anchors.",
  useUiAnchor: "Set UI balloon anchor",
  useCameraAnchor: "Set camera anchor",
  clearName: "Clear name",
  commitName: "Write name",
  resetName: "Revert",
  position: "Local position",
  positionHint: "Use the Transform tab handle to move it.",
  anchorKind: {
    ui: "UI balloon anchor",
    camera: "Camera anchor",
    custom: "Custom name",
    unnamed: "Unnamed",
  },
  anchorKindHint: {
    ui: "0x141EED53: the engine's hard-coded speech-balloon / UI effect anchor; camera zoom resolves it too.",
    camera: "0x9901C8F9: the engine's second hard-coded location space, resolved through the same branch.",
    custom: "Custom name: only vignette / UI assets referencing the same name will use it.",
    unnamed: "Unnamed: no asset can reference it.",
  },
  issue: {
    "anchor-missing":
      "Unnamed anchor: the engine cannot resolve it by name, and the id column truncates here on write.",
    "anchor-duplicate":
      "Duplicate name: anchors with the same name overwrite each other (the engine keys them by name).",
    "transform-missing": "Missing transform: the transform column truncates here.",
    "anchor-invalid": "Invalid name id (must be a non-zero 32-bit integer).",
  },
  duplicateOf: "Shares its name with {count} anchors",
  legacyTitle: "Legacy columns (not consumed)",
  legacyHint:
    "count / countRandom / agent were once labelled as spawn count and agent reference; they belong to the building-variation system and are unrelated to anchors. Shown read-only, no longer written.",
  castTitle: "Characters & appearance",
  castHint:
    "Who stands here is decided by agents: appearance comes from the transport entry's appearance table (e.g. GI_Simm_politician_ma = outfit 92, the suit). Anchors carry no character data, so renaming one never changes the visible sims.",
  listTitle: "Anchors in this lot",
  listEmpty: "No anchors yet. Drag a preset from the library into the viewport.",
  listSelected: "Selected",
  listSearch: "Search anchor name / id",
  stats: "{total} anchors · {named} named · {unnamed} unnamed",
  statsDuplicate: " · {count} duplicated",
  presetStandard: "Standard anchor",
  presetStandardHint: "Position only, name left blank",
  presetUi: "UI balloon anchor",
  presetUiHint: "Prefilled with 0x141EED53",
  presetCamera: "Camera anchor",
  presetCameraHint: "Prefilled with 0x9901C8F9",
  readonlyHint:
    "Read-only view: unlock editing in the asset editor to write changes.",
  selectAnchor: "Select this anchor",
} as const;
