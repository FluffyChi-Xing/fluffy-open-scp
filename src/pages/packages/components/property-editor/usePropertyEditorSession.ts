import { computed, reactive, ref, shallowRef } from "vue";
import { createDataSource } from "@/api/data-source";
import { parseLotModelContainer } from "@/lib/three-gltf";
import { renderTelemetry } from "@/lib/renderTelemetry";
import { unitId } from "./unitGizmos";
import { createUnitEditLayer, mergeUnitOverrides } from "./unitEditLayer";
import type {
  DecalUnit,
  EffectUnit,
  LightUnit,
  LotEditorSession,
  LotModelLodRef,
  LotModelPayload,
  LotUnitDto,
  PathPointUnit,
  PropUnit,
  SpawnerUnit,
  Tgi,
} from "@/api/tauri";

/** 每次打开会话递增，用于把同一 lot 的多次打开区分为不同遥测会话。 */
let sessionEpoch = 0;

/** Outliner/状态栏共用的 Unit 显示名（灯光优先 DebugName）。 */
export function unitLabel(
  unit: LotUnitDto,
  t: (key: string) => string,
): string {
  switch (unit.kind) {
    case "light":
      return unit.debugName ?? `${t("package.groupLights")} #${unit.index + 1}`;
    case "decal":
      return `${t("package.groupDecals")} ${unit.category + 1}·${unit.index + 1}`;
    case "prop":
      return `${t("package.groupProps")} ${unit.bin}·${unit.index + 1}`;
    case "pathPoint":
      return `${t("package.groupPaths")} #${unit.pointIndex ?? unit.index}`;
    case "effect":
      return `${t("package.groupEffects")} #${unit.index + 1}`;
    case "spawner":
      return `${t("package.groupSpawners")} #${unit.index + 1}`;
  }
}

export type ModelState = "pending" | "loading" | "ready" | "missing" | "error";

export interface UnitGrouping {
  lights: LightUnit[];
  decals: DecalUnit[];
  props: PropUnit[];
  effects: EffectUnit[];
  spawners: SpawnerUnit[];
  pathPoints: PathPointUnit[];
}

/**
 * Property Editor 只读会话：一次性装配属性字典 + LOD1 模型几何 +
 * Unit 分组。任何一项失败不阻塞（模型缺失 = 仅表单模式，同原 SCP 静默
 * 跳过但给出提示）。
 */
export function usePropertyEditorSession(packageId: number, tgi: Tgi) {
  const source = createDataSource();
  const session = shallowRef<LotEditorSession | null>(null);
  const loading = ref(true);
  const loadError = ref("");
  const modelPayload = shallowRef<LotModelPayload | null>(null);
  /** LOD1~LOD4 资源位置；index 0 = LOD1，缺失级为 null。 */
  const modelLods = shallowRef<(LotModelLodRef | null)[]>([]);
  /** 当前加载的 LOD（index）；默认取第一个可用级。 */
  const activeLod = ref(0);
  const modelState = ref<ModelState>("pending");
  const selectedId = ref<string | null>(null);
  /** 本地编辑层（PE-重构-2）：transform override + undo/redo，不写回后端。 */
  const edit = createUnitEditLayer();
  const hiddenUnits = ref(new Set<string>());
  const groupVisibility = reactive<Record<string, boolean>>({
    model: true,
    lot: true,
    lights: true,
    props: true,
    decals: true,
    effects: true,
    spawners: true,
    paths: true,
  });

  let requestToken = 0;
  /** 每次 open 递增，用于把同一 lot 的多次打开区分为不同遥测会话。 */
  const openEpoch = ++sessionEpoch;

  async function load() {
    const token = ++requestToken;
    loading.value = true;
    loadError.value = "";
    session.value = null;
    modelPayload.value = null;
    modelLods.value = [];
    activeLod.value = 0;
    modelState.value = "pending";
    selectedId.value = null;
    edit.reset();
    renderTelemetry.setContext({
      sessionKey: `${packageId}:${tgi.typeId}:${tgi.instance}:${openEpoch}`,
      trigger: "first_load",
    });
    const span = renderTelemetry.begin("texture_compose", {
      packageId,
      instance: tgi.instance,
    });
    try {
      const result = await source.readLotEditorSession(packageId, tgi);
      if (token !== requestToken) return;
      session.value = result;
      modelLods.value = result.modelLods ?? [];
      const firstAvailable = modelLods.value.findIndex((lod) => lod !== null);
      if (firstAvailable >= 0) {
        activeLod.value = firstAvailable;
        void loadLod(token, firstAvailable, "first_load");
      } else {
        modelState.value = "missing";
      }
    } catch {
      if (token !== requestToken) return;
      loadError.value = "propertyEditorLoadFailed";
    } finally {
      span.end();
      if (token === requestToken) loading.value = false;
    }
  }

  /** 加载指定 LOD 的模型几何链：单命令取全部网格 GLB + 材质。失败降级不阻塞。 */
  async function loadLod(
    token: number,
    index: number,
    reason: "first_load" | "lod_switch" = "lod_switch",
  ) {
    const lod = modelLods.value[index];
    if (!lod) return;
    modelState.value = "loading";
    renderTelemetry.setContext({
      sessionKey: `${packageId}:${tgi.typeId}:${tgi.instance}:${openEpoch}`,
      trigger: reason,
    });
    const span = renderTelemetry.begin("model_load", { lod: index });
    let meta: Record<string, unknown> = {};
    try {
      const buffer = await source.readLotModelMeshes(lod.packageId, lod.tgi);
      if (token !== requestToken) return;
      meta.bytes = buffer.byteLength;
      const payload = parseLotModelContainer(buffer);
      meta.glbs = payload.glbs.length;
      if (!payload.glbs.length) {
        modelState.value = "missing";
        return;
      }
      modelPayload.value = payload;
      modelState.value = "ready";
    } catch {
      if (token !== requestToken) return;
      modelState.value = "error";
      meta = { failed: true };
    } finally {
      span.end(meta);
    }
  }

  /** 切换 LOD：同级别幂等；请求中忽略新切换（requestToken 已防竞态）。 */
  function switchLod(index: number) {
    if (index === activeLod.value || !modelLods.value[index]) return;
    activeLod.value = index;
    void loadLod(++requestToken, index, "lod_switch");
  }

  const grouping = computed<UnitGrouping>(() => {
    const units = mergeUnitOverrides(
      (session.value?.units ?? []) as LotUnitDto[],
      edit.overrides,
      edit.fieldOverrides,
    );
    const result: UnitGrouping = {
      lights: [],
      decals: [],
      props: [],
      effects: [],
      spawners: [],
      pathPoints: [],
    };
    for (const unit of units as LotUnitDto[]) {
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
  });

  const flatUnits = computed<LotUnitDto[]>(() =>
    mergeUnitOverrides(
      (session.value?.units ?? []) as LotUnitDto[],
      edit.overrides,
      edit.fieldOverrides,
    ),
  );

  const lotTilePeriod = computed<[number, number] | null>(
    () => session.value?.lotTilePeriod ?? null,
  );
  const lotSize = computed<[number, number] | null>(
    () => session.value?.lotSize ?? null,
  );

  const lotPlacement = computed<number[] | null>(() => {
    const matrix = session.value?.lotPlacement;
    return matrix && matrix.length === 12 ? matrix : null;
  });

  const lotColorsAuthored = computed<boolean[]>(() => {
    return session.value?.lotColorsAuthored ?? [false, false, false, false];
  });

  const lotColors = computed<[number, number, number, number][]>(() => {
    return (
      session.value?.lotColors ?? [
        [0, 0, 0, 0],
        [255, 0, 0, 0],
        [0, 255, 0, 0],
        [0, 0, 255, 0],
      ]
    );
  });

  const lotBorderColors = computed<[number, number, number][]>(() => {
    return (
      session.value?.lotBorderColors ?? [
        [156, 156, 156],
        [156, 156, 156],
        [156, 156, 156],
        [156, 156, 156],
      ]
    );
  });

  const lotBorderWidths = computed<number[]>(() => {
    const widths = session.value?.lotBorderWidths;
    return widths && widths.length === 4 ? widths : [0, 0, 0, 0];
  });

  const lotOverlayBoxOffset = computed<[number, number] | null>(() => {
    return session.value?.lotOverlayBoxOffset ?? null;
  });

  const lotModelBBoxCenter = computed<[number, number] | null>(() => {
    return session.value?.lotModelBBoxCenter ?? null;
  });

  const lotSurfacePng = computed<string | null>(() => {
    const png = session.value?.lotSurfacePng;
    return png ? `data:image/png;base64,${png}` : null;
  });

  const lotTintAtlasPng = computed<string | null>(() => {
    const png = session.value?.lotTintAtlasPng;
    return png ? `data:image/png;base64,${png}` : null;
  });

  const lotNormalAtlasPng = computed<string | null>(() => {
    const png = session.value?.lotNormalAtlasPng;
    return png ? `data:image/png;base64,${png}` : null;
  });

  const lotMaskRawRgba = computed<string | null>(() => {
    return session.value?.lotMaskRawRgba ?? null;
  });

  const lotMaskPng = computed<string | null>(() => {
    const png = session.value?.lotMaskPng;
    // 后端返回裸 base64,TextureLoader 需要 data URL。
    return png ? `data:image/png;base64,${png}` : null;
  });

  const lotAlbedoPng = computed<string | null>(() => {
    const png = session.value?.lotAlbedoPng;
    return png ? `data:image/png;base64,${png}` : null;
  });

  const selectedUnit = computed<LotUnitDto | null>(() => {
    if (!selectedId.value) return null;
    return (
      flatUnits.value.find((unit) => unitId(unit) === selectedId.value) ?? null
    );
  });

  function isUnitHidden(unit: LotUnitDto) {
    return hiddenUnits.value.has(unitId(unit));
  }

  function toggleUnit(unit: LotUnitDto) {
    const id = unitId(unit);
    const next = new Set(hiddenUnits.value);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    hiddenUnits.value = next;
  }

  function toggleGroup(name: string) {
    groupVisibility[name] = !groupVisibility[name];
  }

  return {
    session,
    loading,
    loadError,
    modelPayload,
    modelLods,
    activeLod,
    switchLod,
    modelState,
    selectedId,
    grouping,
    flatUnits,
    lotSize,
    lotTilePeriod,
    lotPlacement,
    lotColors,
    lotColorsAuthored,
    lotBorderColors,
    lotBorderWidths,
    lotOverlayBoxOffset,
    lotModelBBoxCenter,
    lotMaskPng,
    lotMaskRawRgba,
    lotAlbedoPng,
    lotSurfacePng,
    lotTintAtlasPng,
    lotNormalAtlasPng,
    selectedUnit,
    edit,
    hiddenUnits,
    groupVisibility,
    load,
    isUnitHidden,
    toggleUnit,
    toggleGroup,
  };
}
