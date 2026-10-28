import { computed, nextTick, reactive, ref, shallowRef, watch } from "vue";
import type { LotModelPayload, LotUnitDto, PropUnit, Tgi } from "@/api/tauri";
import { tauriApi } from "@/api";
import { command } from "@/api/tauri";
import {
  inspectGlb,
  normalizeLotModelScene,
  parseLotModelContainer,
  releaseLotModelCache,
} from "@/lib/three-gltf";
import { createDataSource } from "@/api/data-source";
import {
  clonePropForPlacement,
  directModelKey,
  placementMatrix,
} from "@/pages/packages/components/property-editor/assetCategories";
import {
  buildSchemaDoc,
  unitsFromSchema,
} from "@/pages/packages/components/property-editor/peSchemaDoc";
import { unitId } from "@/pages/packages/components/property-editor/unitGizmos";
import { createUnitEditLayer, mergeUnitOverrides } from "@/pages/packages/components/property-editor/unitEditLayer";
import {
  adoptOrphanPaths,
  nextPathId,
  recomputePathTangents,
  type Vec3,
  type WorkbenchPath,
  type WorkbenchPathSource,
} from "./path-planning/pathModel";
import {
  usePropertyEditorSession,
  type ModelState,
  type UnitGrouping,
} from "@/pages/packages/components/property-editor/usePropertyEditorSession";

/**
 * Mod 资产工作台统一会话（Blender 式重构 2026-10-25）。
 *
 * 两种资产来源共用同一套编辑契约（视口/组件树/检查器/Schema 消费面完全
 * 一致，与 PropertyEditor 会话层同构）：
 *  - native：TGI 直接解析游戏包（packages.open → readLotEditorSession 全链）；
 *  - draft：外部导入 GLB/LOTM（Blender 六槽烘焙管线产物，自动统一到视口 Y-up）。
 *
 * 文档持久化 = 项目内 `asset.lot.json`（openscp.lot-asset/1 + editor.workbench
 * 来源块）；打开工作台自动恢复，保存按钮写回，导出走系统对话框。
 */

export type WorkbenchSourceKind = "none" | "glb" | "lotm" | "native";

/** editor.workbench 扩展块：会话恢复所需的来源信息。 */
interface WorkbenchSourceBlock {
  source: "glb" | "lotm" | "native";
  assetPath?: string;
  packagePath?: string;
  tgi?: Tgi;
  lotSize?: [number, number] | null;
  menu?: WorkbenchMenuBinding;
  /** 路径实体（entry/scope/roadConnect + 有序点引用；几何在 pathPoint 单元）。 */
  paths?: WorkbenchPath[];
  /** 草稿模型伪单元变换（行主序 12 floats）。 */
  modelTransform?: number[] | null;
  /** 六槽材质绑定（项目内相对路径）。 */
  materialSlots?: MaterialSlotBinding | null;
  lotStyle?: {
    colors: [number, number, number, number][];
    colorsAuthored: boolean[];
    borderColors: [number, number, number][];
    borderWidths: number[];
    borderPatternIndices: number[];
    baseTile: number;
    tilePeriod: [number, number] | null;
    /** 来源 Lot 引用：恢复时重新拉取会话取回 mask/albedo/surface 等贴图
     * （贴图 base64 不入文档——code_write_text 有 4MiB 上限）。 */
    sourceLot?: LotPresetRef | null;
  };
}

export interface WorkbenchMenuBinding {
  enabled: boolean;
  label: string;
  category: string;
}

/** 六槽材质绑定：项目内资产相对路径（params.json + slot1..5.png）。 */
export interface MaterialSlotBinding {
  params?: string;
  materialParams?: string;
  slot1?: string;
  slot2?: string;
  slot3?: string;
  slot4?: string;
  slot5?: string;
  /** 外部 .blend 按 GLB 材质名保存的独立槽位。 */
  byMaterial?: Record<string, Omit<MaterialSlotBinding, "byMaterial">>;
}

/** 组件库拖放载荷（与 PropertyEditor 放置契约同构）。 */
export interface PlacePayload {
  kind: "light" | "prop" | "spawner" | "effect" | "pathPoint";
  lightType?: "Point" | "Spot" | "Line";
  packageId?: number;
  packagePath?: string;
  tgi?: Tgi;
  name?: string;
  template?: PropUnit;
}

/** Lot 库应用载荷：Lot 资源位置。 */
export interface LotPresetRef {
  packageId: number;
  tgi: Tgi;
}

const DOCUMENT_PATH = "asset.lot.json";

/** 草稿模式的 TGI 占位：typeId 用模型资源类，instance 取文件名 FNV-1a
 * （跨会话稳定，schema meta.source.tgi 可读）。 */
function draftTgi(name: string): Tgi {
  let hash = 0x811c9dc5;
  for (let index = 0; index < name.length; index += 1) {
    hash ^= name.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return { typeId: 0x2f4e681b, group: 0, instance: hash >>> 0 };
}

function base64ToBytes(base64: string): ArrayBuffer {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1)
    bytes[index] = binary.charCodeAt(index);
  return bytes.buffer;
}

const DRAFT_LOT_DEFAULTS = {
  lotColors: [
    [128, 128, 128, 0],
    [128, 128, 128, 0],
    [128, 128, 128, 0],
    [128, 128, 128, 0],
  ] as [number, number, number, number][],
  lotColorsAuthored: [true, true, true, true],
  lotBorderColors: [
    [96, 96, 96],
    [96, 96, 96],
    [96, 96, 96],
    [96, 96, 96],
  ] as [number, number, number][],
  lotBorderWidths: [1, 0, 0, 0],
};

function emptyGrouping(): UnitGrouping {
  return {
    lights: [],
    decals: [],
    props: [],
    effects: [],
    spawners: [],
    pathPoints: [],
  };
}

function groupUnits(units: LotUnitDto[]): UnitGrouping {
  const result = emptyGrouping();
  for (const unit of units) {
    switch (unit.kind) {
      case "light":
        result.lights.push(unit);
        break;
      case "decal":
        result.decals.push(unit);
        break;
      case "prop":
        result.props.push(unit);
        break;
      case "effect":
        result.effects.push(unit);
        break;
      case "spawner":
        result.spawners.push(unit);
        break;
      case "pathPoint":
        result.pathPoints.push(unit);
        break;
    }
  }
  return result;
}

export function useWorkbenchAsset(project: string) {
  // ---- 会话状态（native / draft 二选一激活）----
  const sourceKind = ref<WorkbenchSourceKind>("none");
  const sourceLabel = ref("");
  const documentRevision = ref<string | undefined>(undefined);
  const savedDirty = ref(false);
  const restoring = ref(false);

  const menu = reactive<WorkbenchMenuBinding>({
    enabled: false,
    label: "",
    category: "",
  });

  // draft：GLB/LOTM 载荷 + bbox 推导的地块尺寸（用户可在"地块"页签改）
  const draftPayload = shallowRef<LotModelPayload | null>(null);
  const draftModelState = ref<ModelState>("pending");
  const draftLotSize = ref<[number, number] | null>(null);
  const draftLoading = ref(false);
  const draftError = ref("");
  /** 草稿地块分层样式：默认中性灰；Lot 库应用后替换为该 Lot 的配置。
   * mask/albedo/surface/normal 为 data URL（raw 为裸 base64），来自被应用
   * Lot 的会话数据——没有它们地面只是平色矩形（用户反馈 2026-10-25）。
   * 【持久化口径】贴图本身不入文档（4MiB 写回上限），只存 sourceLot
   * 引用，恢复时经 applyLotPreset 重新拉取。 */
  const appliedLot = ref<LotPresetRef | null>(null);
  const draftStyle = reactive({
    colors: DRAFT_LOT_DEFAULTS.lotColors.map(
      (color) => [...color] as [number, number, number, number],
    ),
    colorsAuthored: [...DRAFT_LOT_DEFAULTS.lotColorsAuthored],
    borderColors: DRAFT_LOT_DEFAULTS.lotBorderColors.map(
      (color) => [...color] as [number, number, number],
    ),
    borderWidths: [...DRAFT_LOT_DEFAULTS.lotBorderWidths],
    borderPatternIndices: [0, 0, 0, 0],
    baseTile: 8,
    tilePeriod: null as [number, number] | null,
    maskPng: null as string | null,
    maskRawRgba: null as string | null,
    albedoPng: null as string | null,
    surfacePng: null as string | null,
    normalAtlasPng: null as string | null,
  });
  const draftSelectedId = ref<string | null>(null);
  /** 草稿模型伪单元（"model"）的变换：行主序 12 floats（手柄四种工具
   * 提交；独立于 unit 编辑层，随文档持久化）。 */
  const draftModelTransform = ref<number[] | null>(null);
  function setModelTransform(matrix: number[]) {
    if (matrix.length !== 12) return;
    draftModelTransform.value = [...matrix];
  }

  /** 读取项目内槽位文件（params.json + slot png）为字节。 */
  async function readSlotBytes(
    path?: string,
  ): Promise<Uint8Array<ArrayBuffer> | null> {
    if (!path) return null;
    const value = await command<{ base64?: string }>("code_flow", {
      request: { project, action: "read-asset", payload: { asset: path } },
    });
    if (!value.base64) return null;
    return new Uint8Array(base64ToBytes(value.base64));
  }

  /**
   * 组装六槽材质（管线契约）：params.json（width + values rows×4 f32）+
   * slot1 色彩控制图 / slot2 法线 / slot3 shader map / slot4 调色板 /
   * slot5 内景图集 → LotMaterialTextures[0]，并把 mesh 标为 uvKind=2
   * （tint 着色器）+ 材质 0——契约 GLB 的 TEXCOORD_1.x=材质列、
   * TEXCOORD_2/3=facade 投影 UV 与游戏约定一致，原生 tint 链直接可用。
   * 全部槽位清空 = 回到 COLOR_0 调色板渲染。
   */
  async function composeSlotMaterials(binding: MaterialSlotBinding) {
    if (sourceKind.value !== "glb" && sourceKind.value !== "lotm") return;
    const composing = ++slotComposeEpoch;
    slotMaterialsComposing.value = true;
    try {
      const base = draftPayload.value;
      if (!base) return;
      let materials: LotModelPayload["materials"] = [];
      let meshUvKinds: number[] = [];
      if (binding.params || binding.slot1 || binding.byMaterial) {
        const [paramsBytes, materialParamsBytes] = await Promise.all([
          readSlotBytes(binding.params),
          readSlotBytes(binding.materialParams),
        ]);
        let paramsF32: Float32Array | null = null;
        let paramCols = 0;
        let paramsByMaterial:
          | Record<
              string,
              { width: number; values: number[]; paletteRow?: number }
            >
          | undefined;
        if (paramsBytes) {
          const parsed = JSON.parse(new TextDecoder().decode(paramsBytes)) as {
            width?: number;
            values?: number[][] | number[];
          };
          const rows = Array.isArray(parsed.values?.[0])
            ? (parsed.values as number[][])
            : [];
          if (rows.length) {
            paramCols = rows.length;
            paramsF32 = new Float32Array(rows.flat());
          } else if (
            typeof parsed.width === "number" &&
            Array.isArray(parsed.values)
          ) {
            const flat = parsed.values as number[];
            paramCols = parsed.width;
            paramsF32 = new Float32Array(flat);
          }
        }
        if (materialParamsBytes) {
          try {
            const parsed = JSON.parse(
              new TextDecoder().decode(materialParamsBytes),
            ) as Record<
              string,
              { width?: number; values?: unknown; paletteRow?: number }
            >;
            const entries = Object.entries(parsed).flatMap(([name, value]) => {
              if (
                !value ||
                typeof value.width !== "number" ||
                !Array.isArray(value.values) ||
                value.values.some((item) => typeof item !== "number")
              ) {
                return [];
              }
              return [
                [
                  name,
                  {
                    width: value.width,
                    values: value.values as number[],
                    paletteRow:
                      typeof value.paletteRow === "number"
                        ? value.paletteRow
                        : undefined,
                  },
                ],
              ] as const;
            });
            if (entries.length) paramsByMaterial = Object.fromEntries(entries);
          } catch {
            paramsByMaterial = undefined;
          }
        }
        const materialBindings = Object.entries(binding.byMaterial ?? {});
        if (materialBindings.length === 0 && (binding.slot1 || binding.slot4)) {
          materialBindings.push(["", binding]);
        }
        for (const [materialName, slotBinding] of materialBindings) {
          const [slot1, slot2, slot3, slot4, slot5] = await Promise.all([
            readSlotBytes(slotBinding.slot1),
            readSlotBytes(slotBinding.slot2),
            readSlotBytes(slotBinding.slot3),
            readSlotBytes(slotBinding.slot4),
            readSlotBytes(slotBinding.slot5),
          ]);
          if (!slot1 || !slot4) continue;
          materials.push({
            materialName: materialName || undefined,
            baseColorPng: null,
            slot0Png: null,
            normalPng: slot2,
            roughnessPng: null,
            aoPng: null,
            tintPng: slot1,
            palettePng: slot4,
            shaderPng: slot3,
            interiorPng: slot5,
            reliefPng: null,
            paramsF32,
            paramCols,
            paramsByMaterial,
          });
        }
        if (materials.length) {
          // Blender GLB 的 UV 语义不同于 LOTM：基础域在 TEXCOORD_1，
          // 立面/Top 域在 TEXCOORD_3；每个 GLB 材质按名称选择自己的槽位。
          // 用独立 uvKind 让视口选择 Blender shader 变体。
          meshUvKinds = base.glbs.map(() =>
            sourceKind.value === "glb" ? 3 : 2,
          );
        }
      }
      if (composing !== slotComposeEpoch) return;
      // 新 payload 身份驱动视口缓存重建（GLB buffer 原样复用）
      draftPayload.value = {
        ...base,
        materials,
        meshMaterialIndices: materials.length
          ? base.glbs.map((_, index) => index < materials.length ? index : 0)
          : [],
        meshUvKinds,
        diagnostics: materials.length
          ? `${base.diagnostics}\n六槽材质已绑定：params${binding.params ? "✓" : "✗"} slot1${binding.slot1 ? "✓" : "✗"} slot2${binding.slot2 ? "✓" : "✗"} slot3${binding.slot3 ? "✓" : "✗"} slot4${binding.slot4 ? "✓" : "✗"} slot5${binding.slot5 ? "✓" : "✗"} → tint 着色器`
          : base.diagnostics,
      };
      draftMaterialSlots.value = { ...binding };
      savedDirty.value = true;
    } finally {
      if (composing === slotComposeEpoch) slotMaterialsComposing.value = false;
    }
  }
  const draftHiddenUnits = ref(new Set<string>());
  const draftGroupVisibility = reactive<Record<string, boolean>>({
    model: true,
    lot: true,
    lights: true,
    props: true,
    decals: false,
    effects: true,
    spawners: true,
    paths: true,
  });
  /** 模型放置原点平移：水平居中 + 底面坐到 y=0（世界空间；由导入时
   * bbox 推导，视口在模型装配时应用——地面恒在原点平面，模型永远坐在
   * 地块上，不再出现地面/尺寸落到模型下方（用户反馈 2026-10-25）。 */
  const draftModelOrigin = ref<[number, number, number] | null>(null);
  /** 六槽材质绑定（管线契约：params.json + slot1..5.png）；绑定后组装
   * LotMaterialTextures 走原生 tint 着色器（契约 GLB 的 TEXCOORD_1.x =
   * 材质列 / TEXCOORD_2..3 = facade 投影 UV，与游戏约定一致）。 */
  const draftMaterialSlots = ref<MaterialSlotBinding>({});
  const slotMaterialsComposing = ref(false);

  // native：完整 PropertyEditor 会话（每次打开 TGI 重建实例）
  const nativeSession = shallowRef<ReturnType<
    typeof usePropertyEditorSession
  > | null>(null);
  const nativeTgi = ref<Tgi | null>(null);

  // ---- 编辑层（draft 专用；native 用会话自带的）----
  const draftEdit = createUnitEditLayer();

  /** 放置 prop 的直挂模型载荷缓存（directModelKey → 载荷）。 */
  const addedModelPayloads = ref(new Map<string, LotModelPayload>());
  const placementSource = createDataSource();

  // ---- 来源信息（恢复用）----
  const sourceAssetPath = ref("");
  const sourcePackagePath = ref("");
  /** native 会话的包 id（特效预览等需要）。 */
  const nativePackageId = ref<number | null>(null);
  /** 项目根绝对路径（.package 树项 → packages.open 需要绝对路径）。 */
  let projectRootPath = "";
  async function resolveProjectRoot(): Promise<string> {
    if (projectRootPath) return projectRootPath;
    const tree = await tauriApi.workspace.codeTree(project);
    projectRootPath = tree.rootPath;
    return projectRootPath;
  }

  /** 相对路径 → 绝对路径（Windows 反斜杠根与 POSIX 根都兼容）。 */
  function joinProjectPath(relative: string): string {
    const separator = /[\\/]/.test(projectRootPath.slice(-1))
      ? ""
      : projectRootPath.includes("\\")
        ? "\\"
        : "/";
    return `${projectRootPath}${separator}${relative}`;
  }

  let loadEpoch = 0;
  /** 六槽组装代数：连续绑定/解绑时旧组装不得覆盖新 payload。 */
  let slotComposeEpoch = 0;

  /** draft 生效单元集 = edit 层新增（滤软删除），并应用变换/字段 override。
   * 不合并 overrides 时：元数据 scale 等字段编辑与拖动 transform 提交只进
   * 编辑层 override 表，画布永远读不到（2026-10-27 缩放不生效/拖动回原点根因）。 */
  const draftEffectiveUnits = computed<LotUnitDto[]>(() =>
    mergeUnitOverrides(
      draftEdit.addedUnits.filter(
        (unit) => !draftEdit.deletedIds.has(unitId(unit)),
      ),
      draftEdit.overrides,
      draftEdit.fieldOverrides,
    ),
  );

  /** 当前激活的编辑面：模板/子组件统一从这里取数。 */
  const ui = computed(() => {
    if (sourceKind.value === "native" && nativeSession.value) {
      const session = nativeSession.value;
      return {
        editMode: "native" as const,
        loading: session.loading.value,
        loadError: session.loadError.value,
        modelPayload: session.modelPayload.value,
        modelLods: session.modelLods.value,
        activeLod: session.activeLod.value,
        modelState: session.modelState.value,
        grouping: session.grouping.value,
        flatUnits: session.flatUnits.value,
        selectedUnit: session.selectedUnit.value,
        edit: session.edit,
        hiddenUnits: session.hiddenUnits.value,
        groupVisibility: session.groupVisibility,
        lotSize: session.lotSize.value,
        lotTilePeriod: session.lotTilePeriod.value,
        lotPlacement: session.lotPlacement.value,
        lotColors: session.lotColors.value,
        lotColorsAuthored: session.lotColorsAuthored.value,
        lotBorderColors: session.lotBorderColors.value,
        lotBorderWidths: session.lotBorderWidths.value,
        lotBorderPatternIndices: session.lotBorderPatternIndices.value,
        lotBaseTile: session.lotBaseTile.value,
        lotOverlayBoxOffset: session.lotOverlayBoxOffset.value,
        lotModelBBoxCenter: session.lotModelBBoxCenter.value,
        lotMaskPng: session.lotMaskPng.value,
        lotMaskRawRgba: session.lotMaskRawRgba.value,
        lotAlbedoPng: session.lotAlbedoPng.value,
        lotSurfacePng: session.lotSurfacePng.value,
        lotNormalAtlasPng: session.lotNormalAtlasPng.value,
        decalTextures: session.session.value?.decalTextures ?? [],
        decalLight: session.decalLight.value,
        propModels: session.propModels.value,
        propTreeIds: session.propTreeIds.value,
        treeModelPayloads: session.treeModelPayloads.value,
        treeAtlasPng: session.treeAtlasPng.value,
        referencedModels: session.referencedModels.value,
        selectedId: session.selectedId,
        switchLod: session.switchLod,
        toggleUnit: session.toggleUnit,
        toggleGroup: session.toggleGroup,
        modelOrigin: null,
        materialSlots: null,
        modelTransform: null,
        assetName: session.session.value?.assetName ?? sourceLabel.value,
      };
    }
    if (sourceKind.value === "glb" || sourceKind.value === "lotm") {
      const units = draftEffectiveUnits.value;
      return {
        editMode: "draft" as const,
        loading: draftLoading.value,
        loadError: draftError.value,
        modelPayload: draftPayload.value,
        modelLods: [],
        activeLod: 0,
        modelState: draftModelState.value,
        grouping: groupUnits(viewUnits(units)),
        flatUnits: units,
        selectedUnit:
          units.find((unit) => unitId(unit) === draftSelectedId.value) ?? null,
        edit: draftEdit,
        hiddenUnits: draftHiddenUnits.value,
        groupVisibility: draftGroupVisibility,
        lotSize: draftLotSize.value,
        lotTilePeriod: draftStyle.tilePeriod,
        lotPlacement: null,
        modelOrigin: draftModelOrigin.value,
        materialSlots: { ...draftMaterialSlots.value },
        lotColors: draftStyle.colors,
        lotColorsAuthored: draftStyle.colorsAuthored,
        lotBorderColors: draftStyle.borderColors,
        lotBorderWidths: draftStyle.borderWidths,
        lotBorderPatternIndices: draftStyle.borderPatternIndices,
        lotBaseTile: draftStyle.baseTile,
        lotOverlayBoxOffset: null,
        lotModelBBoxCenter: null,
        lotMaskPng: draftStyle.maskPng,
        lotMaskRawRgba: draftStyle.maskRawRgba,
        lotAlbedoPng: draftStyle.albedoPng,
        lotSurfacePng: draftStyle.surfacePng,
        lotNormalAtlasPng: draftStyle.normalAtlasPng,
        decalTextures: [],
        decalLight: null,
        propModels: new Map<number, LotModelPayload>(),
        propTreeIds: new Set<number>(),
        treeModelPayloads: [],
        treeAtlasPng: null,
        referencedModels: [],
        selectedId: draftSelectedId,
        switchLod: () => {},
        toggleUnit: (unit: LotUnitDto, groupKey?: string) => {
          if (groupKey && draftGroupVisibility[groupKey] === false) {
            draftGroupVisibility[groupKey] = true;
            return;
          }
          const id = unitId(unit);
          const next = new Set(draftHiddenUnits.value);
          if (next.has(id)) next.delete(id);
          else next.add(id);
          draftHiddenUnits.value = next;
        },
        toggleGroup: (name: string) => {
          draftGroupVisibility[name] = !draftGroupVisibility[name];
        },
        modelTransform: draftModelTransform.value,
        assetName: sourceLabel.value,
      };
    }
    return null;
  });

  // ---- 活体 schema 文档（Monaco/保存/蓝图三方同源）----
  const tgi = computed<Tgi>(() => {
    if (sourceKind.value === "native" && nativeTgi.value)
      return nativeTgi.value;
    return draftTgi(sourceLabel.value || "draft");
  });
  const schemaDocument = computed<Record<string, unknown> | null>(() => {
    const current = ui.value;
    if (!current) return null;
    const document = buildSchemaDoc({
      assetName: current.assetName || null,
      tgi: tgi.value,
      modelLods: current.modelLods,
      lotSize: current.lotSize,
      lotTilePeriod: current.lotTilePeriod,
      lotPlacement: current.lotPlacement,
      lotBaseTile: current.lotBaseTile,
      lotColors: current.lotColors,
      lotColorsAuthored: current.lotColorsAuthored,
      lotBorderColors: current.lotBorderColors,
      lotBorderWidths: current.lotBorderWidths,
      lotBorderPatternIndices: current.lotBorderPatternIndices,
      lotOverlayBoxOffset: current.lotOverlayBoxOffset,
      lotModelBBoxCenter: current.lotModelBBoxCenter,
      units: current.flatUnits,
      hiddenUnitIds: [...current.hiddenUnits],
      groups: { ...current.groupVisibility },
    });
    const editor = document.editor as Record<string, unknown>;
    editor.workbench = {
      source: sourceKind.value as WorkbenchSourceBlock["source"],
      assetPath: sourceAssetPath.value || undefined,
      packagePath: sourcePackagePath.value || undefined,
      tgi:
        sourceKind.value === "native" && nativeTgi.value
          ? nativeTgi.value
          : undefined,
      lotSize: sourceKind.value === "native" ? undefined : draftLotSize.value,
      modelTransform:
        sourceKind.value === "native" ? undefined : draftModelTransform.value,
      materialSlots:
        sourceKind.value === "native" ? undefined : draftMaterialSlots.value,
      lotStyle:
        sourceKind.value === "native"
          ? undefined
          : {
              colors: draftStyle.colors.map(
                (color) => [...color] as [number, number, number, number],
              ),
              colorsAuthored: [...draftStyle.colorsAuthored],
              borderColors: draftStyle.borderColors.map(
                (color) => [...color] as [number, number, number],
              ),
              borderWidths: [...draftStyle.borderWidths],
              borderPatternIndices: [...draftStyle.borderPatternIndices],
              baseTile: draftStyle.baseTile,
              tilePeriod: draftStyle.tilePeriod
                ? ([...draftStyle.tilePeriod] as [number, number])
                : null,
              // 贴图（mask/albedo/surface/normal，base64 可达数 MB）不入
              // 文档——code_write_text 有 4MiB 上限（用户实测 limit_exceeded）；
              // 持久化来源 Lot 引用，恢复时重新拉取会话数据。
              sourceLot: appliedLot.value ? { ...appliedLot.value } : null,
            },
      menu: { ...menu },
      // 路径实体（2026-10-27 路径规划）：几何在 pathPoint 单元，这里存
      // entry/scope/roadConnect 与有序点引用；恢复见 restoreUnits。
      paths: draftPaths.value.map((path) => ({ ...path })),
    } satisfies WorkbenchSourceBlock;
    return document;
  });
  const schemaJson = computed(() =>
    schemaDocument.value ? JSON.stringify(schemaDocument.value, null, 2) : "",
  );
  // 内容比对式脏标记：加载/恢复期间的重算（引用变化但内容相同）不算
  // 修改；只有与上次保存基线内容不一致才置脏
  const savedBaseline = ref("");
  watch(schemaDocument, (document) => {
    const json = document ? JSON.stringify(document) : "";
    if (
      restoring.value ||
      draftLoading.value ||
      nativeSession.value?.loading.value
    ) {
      savedBaseline.value = json;
      return;
    }
    if (json !== savedBaseline.value) savedDirty.value = true;
  });

  function resetEditState() {
    releaseLotModelCache();
    draftEdit.reset();
    addedModelPayloads.value = new Map();
    draftSelectedId.value = null;
    draftModelTransform.value = null;
    draftHiddenUnits.value = new Set();
    resetDraftStyle();
    Object.assign(draftGroupVisibility, {
      model: true,
      lot: true,
      lights: true,
      props: true,
      decals: false,
      effects: true,
      spawners: true,
      paths: true,
    });
  }

  // ---- native 打开：包路径 + TGI ----
  async function openNative(packagePath: string, tgi: Tgi) {
    const epoch = ++loadEpoch;
    nativeSession.value = null;
    nativeTgi.value = null;
    resetEditState();
    draftPayload.value = null;
    draftModelState.value = "pending";
    draftLoading.value = false;
    draftError.value = "";
    draftLotSize.value = null;
    draftModelOrigin.value = null;
    draftMaterialSlots.value = {};
    sourceKind.value = "none";
    // 树里来的相对路径 → 项目根绝对路径
    const absolute = /^[A-Za-z]:[\\/]|^[\\/]/.test(packagePath)
      ? packagePath
      : joinProjectPath(packagePath);
    const opened = await tauriApi.packages.open(absolute);
    if (epoch !== loadEpoch) return;
    const packageId = opened.package.packageId;
    nativePackageId.value = packageId;
    const session = usePropertyEditorSession(packageId, tgi);
    nativeSession.value = session;
    nativeTgi.value = tgi;
    sourceKind.value = "native";
    sourceLabel.value = `0x${tgi.instance.toString(16).padStart(8, "0").toUpperCase()}`;
    sourcePackagePath.value = packagePath;
    sourceAssetPath.value = "";
    await session.load();
    // 加载基线落定：待挂起的文档 watcher 刷过之后清脏
    if (epoch === loadEpoch)
      void nextTick().then(() => {
        if (epoch === loadEpoch) {
          savedDirty.value = false;
          savedBaseline.value = schemaJson.value;
        }
      });
  }

  // ---- draft 打开：项目内模型资产（import-asset 已落盘）----
  async function openDraftAsset(assetRelativePath: string) {
    const epoch = ++loadEpoch;
    nativeSession.value = null;
    nativeTgi.value = null;
    resetEditState();
    sourceKind.value = "none";
    draftLoading.value = true;
    draftError.value = "";
    draftPayload.value = null;
    draftModelState.value = "loading";
    draftLotSize.value = null;
    draftModelOrigin.value = null;
    draftMaterialSlots.value = {};
    sourceAssetPath.value = assetRelativePath;
    sourcePackagePath.value = "";
    const name = assetRelativePath.split(/[\\/]/).pop() ?? assetRelativePath;
    sourceLabel.value = name.replace(/\.[^.]+$/, "");
    try {
      const value = await command<{
        asset: string;
        path: string;
        size: number;
        base64?: string;
      }>("code_flow", {
        request: {
          project,
          action: "read-asset",
          payload: { asset: assetRelativePath },
        },
      });
      if (epoch !== loadEpoch) return;
      const bytes = base64ToBytes(value.base64 ?? "");
      const lower = name.toLowerCase();
      let payload: LotModelPayload;
      if (lower.endsWith(".lotm")) {
        payload = parseLotModelContainer(bytes);
      } else {
        // 结构侦测：上轴自动判定 + 材质/贴图/COLOR_0 事实（白模诊断）。
        // 契约 GLB（探针导出：0 材质 0 贴图 + COLOR_0 顶点色）在视口按
        // COLOR_0 调色板顶点色渲染，与游戏调色观感一致。
        const inspection = inspectGlb(bytes);
        payload = {
          glbs: [bytes],
          materials: [],
          meshMaterialIndices: [],
          meshUvKinds: [],
          diagnostics: [
            `GLB · ${inspection.meshCount} mesh · ${inspection.materialCount} 材质 · ${inspection.imageCount} 内嵌贴图`,
            `COLOR_0 顶点色: ${inspection.hasVertexColors ? "有（无贴图时按游戏调色板着色）" : "无"}`,
            `TEXCOORD 组: ${inspection.texcoordSets || 0} · 上轴: ${inspection.upAxis.toUpperCase()}（${inspection.upAxisBasis}）`,
            inspection.imageCount === 0
              ? "契约 GLB 未内嵌贴图——六槽贴图需经 RW4 编译或 TGI 解析原生链路；当前按 COLOR_0 调色板渲染"
              : "内嵌贴图按 glTF 材质直接渲染；烘焙 PBR GLB 不含六槽实时数据（假内景/浮雕/昼夜内景灯），需要这些效果请导入 .blend 源文件（自动走六槽 shader）",
          ].join("\n"),
          upAxis: inspection.upAxis,
          upAxisDeclared: inspection.upAxisDeclared,
        };
      }
      if (!payload.glbs.length) {
        draftModelState.value = "missing";
        return;
      }
      draftPayload.value = payload;
      draftModelState.value = "ready";
      sourceKind.value = lower.endsWith(".lotm") ? "lotm" : "glb";
      const footprint = await measureFootprint(payload);
      if (epoch !== loadEpoch) return;
      draftLotSize.value = footprint.size;
      draftModelOrigin.value = footprint.origin;
      // 恢复已绑定的六槽材质（重开工作台/重载模型后仍生效）
      const binding = { ...draftMaterialSlots.value };
      if (binding.params || binding.slot1) {
        void composeSlotMaterials(binding);
      }
    } catch (cause) {
      if (epoch !== loadEpoch) return;
      draftModelState.value = "error";
      draftError.value = String(cause);
    } finally {
      if (epoch === loadEpoch) {
        draftLoading.value = false;
        // 加载基线落定：待挂起的文档 watcher 刷过之后清脏
        void nextTick().then(() => {
          if (epoch === loadEpoch) {
            savedDirty.value = false;
            savedBaseline.value = schemaJson.value;
          }
        });
      }
    }
  }

  /** 世界空间 bbox → 尺寸 + 放置原点（水平居中 + 底面坐 y=0）。 */
  async function measureFootprint(payload: LotModelPayload): Promise<{
    size: [number, number];
    origin: [number, number, number];
  }> {
    const { GLTFLoader } =
      await import("three/examples/jsm/loaders/GLTFLoader.js");
    const { Box3, Vector3 } = await import("three");
    const loader = new GLTFLoader();
    const box = new Box3();
    let found = false;
    for (const glb of payload.glbs) {
      try {
        const { scene } = await loader.parseAsync(glb, "");
        // 量测必须与渲染（getLotModelObjects）走同一份轴向规范化——
        // 帧一旦分离，modelOrigin 就会把建筑垫离地面（2026-10-26
        // draft 悬空回归根因：量测漏剥根旋转，原点在错误的帧里取值）。
        normalizeLotModelScene(scene, payload.upAxis, {
          declared: payload.upAxisDeclared ?? false,
        });
        scene.updateMatrixWorld(true);
        const part = new Box3().setFromObject(scene);
        if (!Number.isFinite(part.min.x)) continue;
        box.union(part);
        found = true;
      } catch {
        // 单片失败不影响量测其余
      }
    }
    if (!found) return { size: [8, 8], origin: [0, 0, 0] };
    const size = new Vector3();
    box.getSize(size);
    // 量测和装配共享 world 的 Z-up 契约：地面是 XY 平面，Z 是高度。
    // world 根组随后统一绕 X 轴 -90° 显示为 Three.js 的 Y-up。
    const centerX = -(box.min.x + box.max.x) / 2;
    const origin: [number, number, number] = [
      Number(centerX.toFixed(2)),
      Number((-(box.min.y + box.max.y) / 2).toFixed(2)),
      Number((-box.min.z).toFixed(2)),
    ];
    return {
      size: [Math.max(4, Math.ceil(size.x)), Math.max(4, Math.ceil(size.y))],
      origin,
    };
  }

  // ---- 组件放置（组件库 pointer 拖拽 → 视口落点）----
  function nextIndexOf(kind: string): number {
    let max = -1;
    for (const unit of ui.value?.flatUnits ?? []) {
      if (unit.kind === kind && unit.index > max) max = unit.index;
    }
    return max + 1;
  }

  // ---- 路径规划（2026-10-27）：路径为一等实体，几何挂在 pathPoint 单元 ----
  const draftPaths = ref<WorkbenchPath[]>([]);
  /** 绘制中的路径（Draw 模式点击落点追加到它）。 */
  const activePathId = ref<string | null>(null);
  /** 聚焦路径：视口折线只画它（多路径防串线；冻结区视口改造前的前离方案）。 */
  const focusedPathId = ref<string | null>(null);

  function pathById(id: string): WorkbenchPath | null {
    return draftPaths.value.find((path) => path.id === id) ?? null;
  }

  function unitPosition(id: string): Vec3 | null {
    const unit = ui.value?.flatUnits.find((candidate) => unitId(candidate) === id);
    return unit && unit.kind === "pathPoint" ? unit.point : null;
  }

  /** 重算路径全点切线与 pointIndex（引擎要求点/切线等长；经字段 override 走
   * 可撤销编辑层）。 */
  function refreshPathGeometry(path: WorkbenchPath): void {
    const edit = ui.value?.edit;
    if (!edit) return;
    const positions = path.pointIds.map((id) => unitPosition(id));
    const tangents = recomputePathTangents(positions);
    path.pointIds.forEach((id, index) => {
      edit.setUnitFields(id, {
        tangent: tangents[index] ?? null,
        pointIndex: index,
      });
    });
  }

  function createPath(options?: {
    label?: string;
    entry?: string | null;
    source?: WorkbenchPathSource;
  }): string {
    const id = nextPathId(draftPaths.value.map((path) => path.id));
    const count = draftPaths.value.length + 1;
    draftPaths.value = [
      ...draftPaths.value,
      {
        id,
        label: options?.label ?? `#${count}`,
        entry: options?.entry ?? null,
        scope: "local",
        roadConnect: false,
        pointIds: [],
        source: options?.source,
      },
    ];
    activePathId.value = id;
    focusedPathId.value = id;
    return id;
  }

  /** Draw 模式点击落点：把点追加进当前路径（无激活路径则先建一条）。 */
  function addPathPoint(position: [number, number, number]): string | null {
    const edit = ui.value?.edit;
    if (!edit) return null;
    let path = activePathId.value ? pathById(activePathId.value) : null;
    if (!path) {
      const id = createPath();
      path = pathById(id);
    }
    if (!path) return null;
    const unit: LotUnitDto = {
      kind: "pathPoint",
      index: nextIndexOf("pathPoint"),
      point: position,
      tangent: null,
      pointIndex: path.pointIds.length,
      fields: [],
    } as unknown as LotUnitDto;
    edit.addUnit(unit);
    const id = unitId(unit);
    path.pointIds.push(id);
    refreshPathGeometry(path);
    selectUnit(id);
    return id;
  }

  function removeLastPathPoint(): void {
    const path = activePathId.value ? pathById(activePathId.value) : null;
    const edit = ui.value?.edit;
    if (!path || !edit || !path.pointIds.length) return;
    const id = path.pointIds.pop()!;
    edit.removeUnit(id);
    refreshPathGeometry(path);
  }

  function finishPath(): void {
    activePathId.value = null;
  }

  function updatePath(id: string, patch: Partial<WorkbenchPath>): void {
    const path = pathById(id);
    if (!path) return;
    draftPaths.value = draftPaths.value.map((candidate) =>
      candidate.id === id ? { ...candidate, ...patch } : candidate,
    );
  }

  function deletePath(id: string): void {
    const path = pathById(id);
    const edit = ui.value?.edit;
    if (path && edit) for (const pointId of path.pointIds) edit.removeUnit(pointId);
    draftPaths.value = draftPaths.value.filter((candidate) => candidate.id !== id);
    if (activePathId.value === id) activePathId.value = null;
    if (focusedPathId.value === id) focusedPathId.value = null;
  }

  /** 视口分组聚焦：聚焦路径时只画该路径的折线（其余 pathPoint 隐藏出折线，
   * 单元本身仍在 flatUnits/文档里）。 */
  function viewUnits(units: LotUnitDto[]): LotUnitDto[] {
    const focus = focusedPathId.value ? pathById(focusedPathId.value) : null;
    if (!focus) return units;
    const visible = new Set(focus.pointIds);
    return units.filter(
      (unit) => unit.kind !== "pathPoint" || visible.has(unitId(unit)),
    );
  }

  function selectUnit(id: string | null) {
    if (ui.value?.editMode === "draft") draftSelectedId.value = id;
    else if (nativeSession.value) nativeSession.value.selectedId.value = id;
  }

  function placeUnit(
    payload: PlacePayload,
    position: [number, number, number],
  ): string | null {
    const edit = ui.value?.edit;
    if (!edit) return null;
    const transform = {
      matrix: placementMatrix(position, 1),
    };
    let unit: LotUnitDto;
    if (payload.kind === "prop" && payload.template) {
      unit = clonePropForPlacement(
        payload.template,
        nextIndexOf("prop"),
        position,
      );
      unit.displayName = payload.name;
    } else if (payload.kind === "light") {
      unit = {
        kind: "light",
        index: nextIndexOf("light"),
        transform,
        lightType: payload.lightType ?? "Point",
        color: [1, 1, 1],
        outerRadius: 8,
        innerRadius: null,
        diffuse: null,
        length: null,
        cullDistance: "Far",
        isVolumetric: false,
        debugName: null,
        fields: [],
      } as LotUnitDto;
    } else if (
      payload.kind === "prop" &&
      payload.tgi &&
      payload.packageId != null
    ) {
      unit = {
        kind: "prop",
        index: nextIndexOf("prop"),
        bin: 1,
        resourceId: payload.tgi.instance,
        transform,
        slot: null,
        scale: null,
        displayName: payload.name,
        modelTgi: payload.tgi,
        modelPackageId: payload.packageId,
        modelPackagePath: payload.packagePath,
        fields: [],
      } as unknown as LotUnitDto;
    } else if (payload.kind === "spawner") {
      unit = {
        kind: "spawner",
        index: nextIndexOf("spawner"),
        transform,
        id: null,
        count: null,
        countRandom: null,
        agent: null,
        fields: [],
      } as LotUnitDto;
    } else if (payload.kind === "effect") {
      unit = {
        kind: "effect",
        index: nextIndexOf("effect"),
        transform,
        effectId: null,
        externalId: null,
        enabled: true,
        fields: [],
      } as LotUnitDto;
    } else {
      unit = {
        kind: "pathPoint",
        index: nextIndexOf("pathPoint"),
        point: position,
        tangent: null,
        pointIndex: null,
        fields: [],
      } as LotUnitDto;
    }
    edit.addUnit(unit);
    selectUnit(unitId(unit));
    // 直挂 prop：异步取 LOTM 载荷入缓存（渲染器从此读取）
    if (
      payload.kind === "prop" &&
      !payload.template &&
      payload.tgi &&
      payload.packageId != null
    ) {
      const cacheKey = directModelKey({
        modelTgi: payload.tgi,
        modelPackageId: payload.packageId,
      });
      placementSource
        .readLotModelMeshes(payload.packageId, payload.tgi)
        .then((buffer) => {
          const next = new Map(addedModelPayloads.value);
          next.set(cacheKey, parseLotModelContainer(buffer));
          addedModelPayloads.value = next;
        })
        .catch(() => {
          // 载荷失败：标记锥兜底（渲染分支自处理）
        });
    }
    return unitId(unit);
  }

  function deleteUnit(id: string) {
    const edit = ui.value?.edit;
    if (!edit) return;
    edit.removeUnit(id);
    if (ui.value?.selectedId.value === id) selectUnit(null);
  }

  /** 草稿地块尺寸手动调整（地块页签）。 */
  function setDraftLotSize(size: [number, number]) {
    draftLotSize.value = [
      Math.max(4, Math.round(size[0])),
      Math.max(4, Math.round(size[1])),
    ];
  }

  /** Lot 库应用：读取该 Lot 会话并把尺寸/四色分层/边框带/底图格套用到
   * 草稿地块（仅外部导入模型；游戏包资产的地块来自 property，拒绝替换）。
   * 同时载入该 Lot 的 mask/反照率/地表/法线图集——没有这层数据地面只是
   * 平色矩形（默认/精细两种模式都渲染不出分层）。返回该 Lot 的 LotSize。 */
  async function applyLotPreset(
    preset: LotPresetRef,
  ): Promise<[number, number] | null> {
    if (sourceKind.value !== "glb" && sourceKind.value !== "lotm") {
      throw new Error("lot_apply_native");
    }
    const session = await tauriApi.packages.readLotEditorSession(
      preset.packageId,
      preset.tgi,
    );
    if (session.lotSize) {
      draftLotSize.value = [session.lotSize[0], session.lotSize[1]];
    }
    if (session.lotTilePeriod) {
      draftStyle.tilePeriod = [
        session.lotTilePeriod[0],
        session.lotTilePeriod[1],
      ];
    }
    if (session.lotColors?.length === 4) {
      draftStyle.colors = session.lotColors.map(
        (color) => [...color] as [number, number, number, number],
      );
    }
    if (session.lotColorsAuthored?.length === 4) {
      draftStyle.colorsAuthored = [...session.lotColorsAuthored];
    }
    if (session.lotBorderColors?.length === 4) {
      draftStyle.borderColors = session.lotBorderColors.map(
        (color) => [...color] as [number, number, number],
      );
    }
    if (session.lotBorderWidths?.length === 4) {
      draftStyle.borderWidths = [...session.lotBorderWidths];
    }
    if (session.lotBorderPatternIndices?.length === 4) {
      draftStyle.borderPatternIndices = [...session.lotBorderPatternIndices];
    }
    if (Number.isFinite(session.lotBaseTile)) {
      draftStyle.baseTile = session.lotBaseTile;
    }
    draftStyle.maskPng = session.lotMaskPng
      ? `data:image/png;base64,${session.lotMaskPng}`
      : null;
    draftStyle.maskRawRgba = session.lotMaskRawRgba ?? null;
    draftStyle.albedoPng = session.lotAlbedoPng
      ? `data:image/png;base64,${session.lotAlbedoPng}`
      : null;
    draftStyle.surfacePng = session.lotSurfacePng
      ? `data:image/png;base64,${session.lotSurfacePng}`
      : null;
    draftStyle.normalAtlasPng = session.lotNormalAtlasPng
      ? `data:image/png;base64,${session.lotNormalAtlasPng}`
      : null;
    appliedLot.value = { packageId: preset.packageId, tgi: preset.tgi };
    return session.lotSize ?? null;
  }

  function resetDraftStyle() {
    appliedLot.value = null;
    draftStyle.colors = DRAFT_LOT_DEFAULTS.lotColors.map(
      (color) => [...color] as [number, number, number, number],
    );
    draftStyle.colorsAuthored = [...DRAFT_LOT_DEFAULTS.lotColorsAuthored];
    draftStyle.borderColors = DRAFT_LOT_DEFAULTS.lotBorderColors.map(
      (color) => [...color] as [number, number, number],
    );
    draftStyle.borderWidths = [...DRAFT_LOT_DEFAULTS.lotBorderWidths];
    draftStyle.borderPatternIndices = [0, 0, 0, 0];
    draftStyle.baseTile = 8;
    draftStyle.tilePeriod = null;
    draftStyle.maskPng = null;
    draftStyle.maskRawRgba = null;
    draftStyle.albedoPng = null;
    draftStyle.surfacePng = null;
    draftStyle.normalAtlasPng = null;
  }

  function setMenuBinding(patch: Partial<WorkbenchMenuBinding>) {
    Object.assign(menu, patch);
  }

  // ---- 会话恢复：把文档单元投影进 edit 层（与 PropertyEditor 同口径）----
  function restoreUnits(document: Record<string, unknown>) {
    const current = ui.value;
    if (!current) return;
    const restored = unitsFromSchema(document);
    const baseUnits: LotUnitDto[] =
      current.editMode === "native"
        ? ((nativeSession.value?.session.value?.units ?? []) as LotUnitDto[])
        : [];
    const baseIds = new Set(baseUnits.map((unit) => unitId(unit)));
    const edit = current.edit;
    for (const id of baseIds)
      if (!restored.some((unit) => unitId(unit) === id)) edit.removeUnit(id);
    for (const unit of restored) {
      const id = unitId(unit);
      if (!baseIds.has(id)) edit.addUnit(unit);
      else {
        if ("transform" in unit && unit.transform)
          edit.setUnitTransform(id, unit.transform.matrix);
        const { fields, ...patch } = unit;
        edit.setUnitFields(id, { ...patch, fields });
      }
    }
    const editor = document.editor as {
      groups?: Record<string, boolean>;
      unitVisibility?: Record<string, boolean>;
      workbench?: { paths?: WorkbenchPath[] };
    };
    Object.assign(current.groupVisibility, editor?.groups ?? {});
    if (current.editMode === "draft") {
      // 键位与 schemaDocument 写出侧一致：editor.workbench.paths
      draftPaths.value = (editor?.workbench?.paths ?? []).map((path) => ({
        ...path,
        pointIds: [...path.pointIds],
      }));
      // 孤儿点收养：只有点没有路径实体的历史文档，按 pointIndex
      // 自动建一条路径承接（幂等——已被引用的点不会再收）
      const adopted = adoptOrphanPaths(draftPaths.value, restored);
      if (adopted) draftPaths.value = adopted;
      focusedPathId.value = null;
      activePathId.value = null;
    }
    const hidden = new Set(
      Object.entries(editor?.unitVisibility ?? {})
        .filter(([, visible]) => !visible)
        .map(([id]) => id),
    );
    if (current.editMode === "draft") draftHiddenUnits.value = hidden;
    else if (nativeSession.value)
      nativeSession.value.hiddenUnits.value = hidden;
  }

  /** 恢复已放置 prop 的直挂模型载荷（packageId 可能跨运行失效 → 优先按
   * modelPackagePath 重开；失败保持标记锥兜底）。 */
  async function restorePlacedPropModels(document: Record<string, unknown>) {
    let units: LotUnitDto[];
    try {
      units = unitsFromSchema(document);
    } catch {
      return;
    }
    for (const unit of units) {
      if (unit.kind !== "prop" || !unit.modelTgi) continue;
      const row = unit as PropUnit & {
        modelPackageId?: number;
        modelPackagePath?: string;
      };
      const modelTgi = row.modelTgi;
      if (!modelTgi) continue;
      if (row.modelPackageId == null && !row.modelPackagePath) continue;
      try {
        const pkg = row.modelPackagePath
          ? (await tauriApi.packages.open(row.modelPackagePath)).package
              .packageId
          : row.modelPackageId;
        if (pkg == null) continue;
        const bytes = await tauriApi.packages.readLotModelMeshes(pkg, modelTgi);
        const next = new Map(addedModelPayloads.value);
        next.set(
          directModelKey({ modelTgi, modelPackageId: pkg }),
          parseLotModelContainer(bytes),
        );
        addedModelPayloads.value = next;
      } catch {
        // 包缺席：标记锥兜底（渲染分支自处理）
      }
    }
  }

  // ---- 文档持久化（项目内 asset.lot.json）----
  async function saveDocument(): Promise<boolean> {
    const document = schemaDocument.value;
    if (!document) return false;
    const written = await tauriApi.workspace.codeWriteText(
      project,
      DOCUMENT_PATH,
      JSON.stringify(document, null, 2),
      documentRevision.value,
    );
    documentRevision.value = written.revision;
    savedBaseline.value = JSON.stringify(document);
    savedDirty.value = false;
    return true;
  }

  /** 打开工作台时恢复上次会话（文档存在则按来源块重放）。 */
  async function restoreFromProject(): Promise<boolean> {
    restoring.value = true;
    try {
      const document = await tauriApi.workspace.codeReadText(
        project,
        DOCUMENT_PATH,
      );
      documentRevision.value = document.revision;
      const parsed = JSON.parse(document.content) as Record<string, unknown>;
      const workbench = (parsed.editor as Record<string, unknown> | undefined)
        ?.workbench as WorkbenchSourceBlock | undefined;
      if (!workbench) return false;
      menu.enabled = workbench.menu?.enabled ?? false;
      menu.label = workbench.menu?.label ?? "";
      menu.category = workbench.menu?.category ?? "";
      if (
        workbench.source === "native" &&
        workbench.packagePath &&
        workbench.tgi
      ) {
        await openNative(workbench.packagePath, workbench.tgi);
        restoreUnits(parsed);
        void restorePlacedPropModels(parsed);
        return true;
      }
      if (
        (workbench.source === "glb" || workbench.source === "lotm") &&
        workbench.assetPath
      ) {
        await openDraftAsset(workbench.assetPath);
        if (workbench.lotSize) draftLotSize.value = workbench.lotSize;
        draftModelTransform.value =
          workbench.modelTransform && workbench.modelTransform.length === 12
            ? [...workbench.modelTransform]
            : null;
        const slots = workbench.materialSlots;
        if (slots && (slots.params || slots.slot1)) {
          draftMaterialSlots.value = { ...slots };
          void composeSlotMaterials({ ...slots });
        }
        const style = workbench.lotStyle;
        if (style) {
          draftStyle.colors = style.colors.map(
            (color) => [...color] as [number, number, number, number],
          );
          draftStyle.colorsAuthored = [...style.colorsAuthored];
          draftStyle.borderColors = style.borderColors.map(
            (color) => [...color] as [number, number, number],
          );
          draftStyle.borderWidths = [...style.borderWidths];
          draftStyle.borderPatternIndices = [...style.borderPatternIndices];
          draftStyle.baseTile = style.baseTile;
          draftStyle.tilePeriod = style.tilePeriod
            ? ([...style.tilePeriod] as [number, number])
            : null;
          // 来源 Lot 引用 → 重新拉取会话取回 mask/albedo/surface 等贴图
          // （贴图 base64 不入文档，见 lotStyle 持久化口径）
          if (style.sourceLot) {
            appliedLot.value = { ...style.sourceLot };
            await applyLotPreset({ ...style.sourceLot });
            // 手动调过的尺寸以文档为准
            if (workbench.lotSize) draftLotSize.value = workbench.lotSize;
          }
        }
        restoreUnits(parsed);
        void restorePlacedPropModels(parsed);
        return true;
      }
      return false;
    } catch {
      return false;
    } finally {
      restoring.value = false;
      void nextTick().then(() => {
        savedBaseline.value = schemaJson.value;
        savedDirty.value = false;
      });
    }
  }

  /** 关闭当前资产（回到空态）。 */
  function closeAsset() {
    loadEpoch += 1;
    nativeSession.value = null;
    nativeTgi.value = null;
    resetEditState();
    draftPayload.value = null;
    draftModelState.value = "pending";
    draftLotSize.value = null;
    draftModelOrigin.value = null;
    draftMaterialSlots.value = {};
    sourceKind.value = "none";
    sourceLabel.value = "";
    sourceAssetPath.value = "";
    sourcePackagePath.value = "";
    savedDirty.value = false;
  }

  /** 移除当前资产：删除项目内文档与导入的模型文件（外部导入时），回到
   * 空态——用户可重新上传/解析（用户指令 2026-10-25：删除要连文件一起）。 */
  async function removeAsset(): Promise<void> {
    const assetPath = sourceAssetPath.value;
    const hadDocument = Boolean(schemaDocument.value);
    closeAsset();
    // 先删文档再删模型；文档不存在（从未保存过）不算失败
    if (hadDocument) {
      await tauriApi.workspace.codeDeleteFile(project, DOCUMENT_PATH);
    }
    if (assetPath) {
      await tauriApi.workspace.codeDeleteFile(project, assetPath);
    }
  }

  return {
    sourceKind,
    sourceLabel,
    sourceAssetPath,
    sourcePackagePath,
    nativePackageId,
    ui,
    schemaDocument,
    schemaJson,
    menu,
    addedModelPayloads,
    savedDirty,
    restoring,
    documentPath: DOCUMENT_PATH,
    draftLotSize,
    openNative,
    openDraftAsset,
    placeUnit,
    deleteUnit,
    selectUnit,
    draftPaths,
    activePathId,
    focusedPathId,
    createPath,
    addPathPoint,
    removeLastPathPoint,
    finishPath,
    updatePath,
    deletePath,
    setFocusedPath: (id: string | null) => {
      focusedPathId.value = id;
    },
    setDraftLotSize,
    applyLotPreset,
    setModelTransform,
    composeSlotMaterials,
    slotMaterialsComposing,
    draftMaterialSlots,
    setMenuBinding,
    saveDocument,
    restoreFromProject,
    closeAsset,
    removeAsset,
    resolveProjectRoot,
    joinProjectPath,
  };
}

export type WorkbenchAsset = ReturnType<typeof useWorkbenchAsset>;
