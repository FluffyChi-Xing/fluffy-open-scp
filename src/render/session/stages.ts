/**
 * 场景装配阶段（session 雏形）：把“遍历单元 / 投影贴花”的循环从视口搬出。
 *
 * 只做编排（顺序、降级、登记），不持有 UI 状态：所有写回都通过传入的
 * group/unitObjects/stats 引用完成，便于后续升级为完整 RenderSession。
 */
import type * as ThreeNamespace from "three";
import type {
  DecalUnit,
  DecalUnitTexture,
  EffectDefinitionData,
  EffectUnit,
  LightUnit,
  LotUnitDto,
} from "@/api/tauri";
import type { UnitRenderContext } from "@/pages/packages/components/property-editor/renderers/types";
import type {
  ParticleEffectView,
} from "@/pages/packages/components/property-editor/effects/particleRenderer";

export type KindGroup = "model" | "lot" | "dimensions" | "lights" | "props" | "decals" | "effects" | "spawners" | "paths";

export function kindGroup(kind: string): KindGroup {
  switch (kind) {
    case "light":
      return "lights";
    case "prop":
      return "props";
    case "decal":
      return "decals";
    case "effect":
      return "effects";
    case "spawner":
      return "spawners";
    case "pathPoint":
      return "paths";
    default:
      return "model";
  }
}

export interface UnitStageInput {
  readonly THREE: typeof ThreeNamespace;
  readonly units: readonly LotUnitDto[];
  readonly renderMode: "default" | "refined";
  readonly renderEffects: boolean;
  readonly effectDefinitions?: ReadonlyMap<number, EffectDefinitionData>;
  readonly effectsCount: number;
  readonly assetCtx: UnitRenderContext;
  readonly particleViews: ParticleEffectView[];
  readonly createParticleEffect: (
    THREE: typeof ThreeNamespace,
    unit: EffectUnit,
    emitters: EffectDefinitionData["emitters"],
    budget: number,
  ) => ParticleEffectView | null;
  readonly renderAssetUnit: (
    ctx: UnitRenderContext,
    unit: LotUnitDto,
  ) => Promise<ThreeNamespace.Object3D | null>;
  readonly buildFallback: (
    THREE: typeof ThreeNamespace,
    unit: LotUnitDto,
  ) => ThreeNamespace.Object3D | null;
  readonly buildLightUnit: (
    THREE: typeof ThreeNamespace,
    unit: LightUnit,
  ) => ThreeNamespace.Object3D;
  readonly groupFor: (name: KindGroup) => ThreeNamespace.Object3D;
  readonly unitObjects: Map<string, ThreeNamespace.Object3D>;
  readonly unitIdOf: (unit: LotUnitDto) => string;
  readonly isStale: () => boolean;
  readonly yieldToBrowser: () => Promise<void>;
}

/** 非贴花单元：灯光 / 道具 / spawner / 效果 / 路径点。 */
export async function stageUnits(input: UnitStageInput): Promise<void> {
  let sliceStart = performance.now();
  for (const unit of input.units) {
    if (unit.kind === "decal") continue;
    if (performance.now() - sliceStart >= 8) {
      await input.yieldToBrowser();
      sliceStart = performance.now();
      if (input.isStale()) return;
    }
    const effectDefinition =
      unit.kind === "effect" && unit.effectId
        ? input.effectDefinitions?.get(unit.effectId.instance)
        : undefined;
    const particleView =
      input.renderMode === "refined" &&
      input.renderEffects &&
      unit.kind === "effect" &&
      effectDefinition?.emitters.length
        ? input.createParticleEffect(
            input.THREE,
            unit,
            effectDefinition.emitters,
            Math.max(32, Math.min(320, Math.floor(2400 / Math.max(1, input.effectsCount)))),
          )
        : null;
    if (particleView) input.particleViews.push(particleView);
    const object =
      particleView?.object ??
      (input.renderMode === "refined" && unit.kind === "light"
        ? input.buildLightUnit(input.THREE, unit as LightUnit)
        : null) ??
      (await input.renderAssetUnit(input.assetCtx, unit)) ??
      input.buildFallback(input.THREE, unit);
    if (!object) continue;
    object.userData.unitId = input.unitIdOf(unit);
    object.userData.unitKind = unit.kind;
    input.groupFor(kindGroup(unit.kind)).add(object);
    input.unitObjects.set(input.unitIdOf(unit), object);
    if (input.isStale()) return;
  }
}

export interface DecalStageInput {
  readonly THREE: typeof ThreeNamespace;
  readonly renderMode: "default" | "refined";
  readonly decalUnits: readonly DecalUnit[];
  readonly decalTextureByKey: ReadonlyMap<string, DecalUnitTexture>;
  readonly buildingMeshes: readonly ThreeNamespace.Mesh[];
  readonly proxies: readonly ThreeNamespace.Mesh[];
  readonly buildDecalObject: (
    THREE: typeof ThreeNamespace,
    unit: DecalUnit,
    texture: DecalUnitTexture,
    meshes: ThreeNamespace.Mesh[],
    proxies: ThreeNamespace.Mesh[],
  ) => Promise<ThreeNamespace.Object3D | null>;
  readonly buildFallback: (
    THREE: typeof ThreeNamespace,
    unit: LotUnitDto,
  ) => ThreeNamespace.Object3D | null;
  readonly groupFor: (name: KindGroup) => ThreeNamespace.Object3D;
  readonly unitObjects: Map<string, ThreeNamespace.Object3D>;
  readonly unitIdOf: (unit: LotUnitDto) => string;
  readonly isStale: () => boolean;
  readonly onMissingTexture: (unit: DecalUnit, keys: readonly string[]) => void;
}

/** 贴花单元：投影/浮空/降级标记锥。 */
export async function stageDecals(input: DecalStageInput): Promise<void> {
  for (const unit of input.decalUnits) {
    const decalTexture = input.decalTextureByKey.get(`${unit.category}:${unit.index}`);
    if (input.renderMode === "refined" && !decalTexture) {
      input.onMissingTexture(unit, [...input.decalTextureByKey.keys()]);
    }
    let object: ThreeNamespace.Object3D | null;
    const meshes = input.buildingMeshes as ThreeNamespace.Mesh[];
    const proxies = input.proxies as ThreeNamespace.Mesh[];
    if (input.renderMode === "refined" && decalTexture) {
      object =
        (await input.buildDecalObject(input.THREE, unit, decalTexture, meshes, proxies)) ??
        input.buildFallback(input.THREE, unit);
    } else {
      object = input.buildFallback(input.THREE, unit);
    }
    if (!object) continue;
    object.userData.unitId = input.unitIdOf(unit);
    object.userData.unitKind = unit.kind;
    input.groupFor(kindGroup(unit.kind)).add(object);
    input.unitObjects.set(input.unitIdOf(unit), object);
    if (input.isStale()) return;
  }
}
