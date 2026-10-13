import { shallowRef, watch, type Ref } from "vue";
import { createDataSource } from "@/api/data-source";
import type { EffectDefinitionData, EffectUnit } from "@/api/tauri";

export function useEffectPreview(
  packageId: () => number,
  enabled: Ref<boolean>,
  units: Ref<EffectUnit[]>,
) {
  const definitions = shallowRef(new Map<number, EffectDefinitionData>());
  const loading = shallowRef(false);
  const failed = shallowRef(false);
  const limited = shallowRef(false);
  const source = createDataSource();
  let generation = 0;

  watch([enabled, units, packageId], async ([active, current, activePackageId]) => {
    const request = ++generation;
    if (!active) {
      definitions.value = new Map();
      loading.value = false;
      failed.value = false;
      limited.value = false;
      return;
    }
    const ids = [...new Set(current
      .filter((unit) => unit.enabled !== false)
      .map((unit) => unit.effectId?.instance)
      .filter((id): id is number => id != null))];
    if (!ids.length) {
      definitions.value = new Map();
      limited.value = false;
      return;
    }
    loading.value = true;
    failed.value = false;
    try {
      const resolved = new Map<number, EffectDefinitionData>();
      let partial = false;
      for (let offset = 0; offset < ids.length; offset += 64) {
        const response = await source.readEffectPreview(activePackageId, ids.slice(offset, offset + 64));
        if (generation !== request) return;
        partial ||= response.diagnostics.length > 0;
        for (const effect of response.effects) resolved.set(effect.effectId, effect);
      }
      definitions.value = resolved;
      limited.value = partial || ids.some((id) =>
        !resolved.has(id) || resolved.get(id)!.unsupportedBlocks > 0,
      );
    } catch (error) {
      if (generation !== request) return;
      console.error("[effect] Swarm preview failed", error);
      definitions.value = new Map();
      failed.value = true;
      limited.value = false;
    } finally {
      if (generation === request) loading.value = false;
    }
  }, { immediate: true });

  return { definitions, loading, failed, limited };
}
