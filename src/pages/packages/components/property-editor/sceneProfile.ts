/**
 * Property Editor 模块注册机制（PE-重构-1 落地骨架）。
 *
 * 设计（见 docs/roadmap/asset-development.md §二）：编译期静态模块清单，
 * 不做运行时动态注册。每种使用形态（mode）取一份模块清单，壳
 * （PropertyEditor.vue）据此装配 slot 组件 / 热键表 / scene contributor /
 * command。本文件先落地类型与三形态清单；热键与 command 的执行器在
 * PE-重构-2/3 接入。
 */

/** 编辑器 command：可执行、可撤销的操作（PE-重构-2 落地执行器）。 */
export type EditorCommandName =
  | "tool.select"
  | "tool.move"
  | "tool.rotate"
  | "tool.scale"
  | "unit.copy"
  | "unit.paste"
  | "unit.delete";

/** 声明式热键：key 使用小写 + 修饰键（如 "ctrl+c"、"g"）。 */
export interface EditorHotkeyDef {
  key: string;
  command: EditorCommandName;
}

/** 功能开关：壳据此启用形态相关服务（渲染模块/手柄/剪贴板等）。 */
export interface EditorFeatureFlags {
  /** SCP 精细渲染（building4 tint shader + 日夜环境 + 地面量化）。 */
  refinedRender: boolean;
  /** Unit 变换手柄（TransformControls；PE-重构-3）。 */
  transformGizmo: boolean;
  /** Unit 复制粘贴（PE-重构-4）。 */
  clipboard: boolean;
  /** session 本地编辑层（command 栈；PE-重构-2）。 */
  unitEditing: boolean;
  /** 热键启用（含 Esc 降级选中）。 */
  hotkeys: boolean;
}

/** 一个可注册模块：目前承载热键表；slot 组件/scene contributor 在后续
 * 步骤中以组件与 rebuild 装配回调的形式加入。 */
export interface PropertyEditorModule {
  id: string;
  hotkeys?: EditorHotkeyDef[];
}

export type PropertyEditorMode = "preview" | "property-edit" | "rw4-edit";

export interface PropertyEditorProfile {
  mode: PropertyEditorMode;
  features: EditorFeatureFlags;
  modules: PropertyEditorModule[];
}

/** 预览形态：与 PropertyPreview.vue 的只读用法一致（零编辑模块）。 */
const previewProfile: PropertyEditorProfile = {
  mode: "preview",
  features: {
    refinedRender: true,
    transformGizmo: false,
    clipboard: false,
    unitEditing: false,
    hotkeys: false,
  },
  modules: [],
};

/** property 编辑形态（M-PE2 目标）：unit 变换/增删 + 热键 + 手柄。 */
const propertyEditProfile: PropertyEditorProfile = {
  mode: "property-edit",
  features: {
    refinedRender: true,
    transformGizmo: true,
    clipboard: true,
    unitEditing: true,
    hotkeys: true,
  },
  modules: [
    {
      id: "unit-transform-tools",
      hotkeys: [
        { key: "g", command: "tool.move" },
        { key: "r", command: "tool.rotate" },
        { key: "s", command: "tool.scale" },
        { key: "ctrl+c", command: "unit.copy" },
        { key: "ctrl+v", command: "unit.paste" },
      ],
    },
  ],
};

/** rw4 编辑形态（资产开发主线）：占位，范围随 资产-1/2 设计确定。 */
const rw4EditProfile: PropertyEditorProfile = {
  mode: "rw4-edit",
  features: {
    refinedRender: true,
    transformGizmo: true,
    clipboard: true,
    unitEditing: true,
    hotkeys: true,
  },
  modules: [],
};

export const EDITOR_PROFILES: Record<
  PropertyEditorMode,
  PropertyEditorProfile
> = {
  preview: previewProfile,
  "property-edit": propertyEditProfile,
  "rw4-edit": rw4EditProfile,
};
