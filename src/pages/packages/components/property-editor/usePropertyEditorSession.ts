import { computed, reactive, ref, shallowRef } from "vue";
import { createDataSource } from "@/api/data-source";
import { parseLotModelContainer } from "@/lib/three-gltf";
import { unitId } from "./unitGizmos";
import type {
  DecalUnit,
  EffectUnit,
  LightUnit,
  LotEditorSession,
  LotModelPayload,
  LotUnitDto,
  PathPointUnit,
  PropUnit,
  SpawnerUnit,
  Tgi,
} from "@/api/tauri";

/** Outliner/状态栏共用的 Unit 显示名（灯光优先 DebugName）。 */
export function unitLabel(unit: LotUnitDto, t: (key: string) => string): string {
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
  const modelState = ref<ModelState>("pending");
  const selectedId = ref<string | null>(null);
  const hiddenUnits = ref(new Set<string>());
  const groupVisibility = reactive<Record<string, boolean>>({
    model: true,
    lights: true,
    props: true,
    decals: true,
    effects: true,
    spawners: true,
    paths: true,
  });

  let requestToken = 0;

  async function load() {
    const token = ++requestToken;
    loading.value = true;
    loadError.value = "";
    session.value = null;
    modelPayload.value = null;
    modelState.value = "pending";
    selectedId.value = null;
    try {
      const result = await source.readLotEditorSession(packageId, tgi);
      if (token !== requestToken) return;
      session.value = result;
      if (result.modelAvailable && result.modelKey) {
        void loadModel(token, result.modelKey);
      } else {
        modelState.value = "missing";
      }
    } catch {
      if (token !== requestToken) return;
      loadError.value = "propertyEditorLoadFailed";
    } finally {
      if (token === requestToken) loading.value = false;
    }
  }

  /** LOD1 模型几何链：单次命令取全部网格 GLB（含顶点色）+ 材质资源。失败降级不阻塞。 */
  async function loadModel(token: number, modelKey: Tgi) {
    modelState.value = "loading";
    try {
      const buffer = await source.readLotModelMeshes(packageId, modelKey);
      if (token !== requestToken) return;
      const payload = parseLotModelContainer(buffer);
      if (!payload.glbs.length) {
        modelState.value = "missing";
        return;
      }
      modelPayload.value = payload;
      modelState.value = "ready";
    } catch {
      if (token !== requestToken) return;
      modelState.value = "error";
    }
  }

  const grouping = computed<UnitGrouping>(() => {
    const units = session.value?.units ?? [];
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

  const flatUnits = computed<LotUnitDto[]>(() => session.value?.units ?? []);

  const lotSize = computed<[number, number] | null>(
    () => session.value?.lotSize ?? null,
  );

  const lotPlacement = computed<number[] | null>(() => {
    const matrix = session.value?.lotPlacement;
    return matrix && matrix.length === 12 ? matrix : null;
  });

  const lotMaskPng = computed<string | null>(() => {
    const png = session.value?.lotMaskPng;
    // 后端返回裸 base64,TextureLoader 需要 data URL。
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
    modelState,
    selectedId,
    grouping,
    flatUnits,
    lotSize,
    lotPlacement,
    lotMaskPng,
    selectedUnit,
    hiddenUnits,
    groupVisibility,
    load,
    isUnitHidden,
    toggleUnit,
    toggleGroup,
  };
}
