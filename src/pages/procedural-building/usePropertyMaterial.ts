import { computed, onScopeDispose, ref, shallowRef, watch } from "vue";
import { createDataSource } from "@/api/data-source";
import type { LotEditorSession } from "@/api/tauri";
import { useGamePackagesStore } from "@/stores/gamePackages";
import { parseLotModelContainer } from "@/lib/three-gltf";
import {
  analyzeBuildingMaterial,
  parsePropertyTgi,
  type BuildingMaterialSource,
} from "@/lib/procedural-building/propertyMaterial";
import type { PropertyMaterialBinding } from "@/lib/procedural-building/schema";

export function usePropertyMaterial() {
  const packages = useGamePackagesStore();
  const dataSource = createDataSource();
  const input = ref("");
  const loading = ref(false);
  const errorKey = ref("");
  const resolved = shallowRef<{
    source: BuildingMaterialSource;
    binding: PropertyMaterialBinding;
    name: string;
  } | null>(null);
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const packageIds = computed(() =>
    packages.opened.map((entry) => entry.package.packageId),
  );

  async function resolve() {
    clearTimeout(timer);
    const token = ++generation;
    resolved.value = null;
    errorKey.value = "";
    loading.value = false;
    if (!input.value.trim()) return;
    const tgi = parsePropertyTgi(input.value);
    if (!tgi) {
      errorKey.value = "propertyInvalidTgi";
      return;
    }
    if (!packageIds.value.length) {
      errorKey.value = "propertyNoPackages";
      return;
    }
    loading.value = true;
    try {
      const ordered = [...packages.opened].sort(
        (a, b) =>
          Number(b.package.packageId === packages.activePackageId) -
          Number(a.package.packageId === packages.activePackageId),
      );
      for (const entry of ordered) {
        let session: LotEditorSession;
        try {
          session = await dataSource.readLotEditorSession(
            entry.package.packageId,
            tgi,
          );
        } catch {
          if (token !== generation) return;
          else continue;
        }
        if (token !== generation) return;
        const lodIndex = session.modelLods.findIndex((lod) => lod !== null);
        const lod = session.modelLods[lodIndex];
        if (!lod) {
          errorKey.value = "propertyNoModel";
          return;
        }
        const buffer = await dataSource.readLotModelMeshes(
          lod.packageId,
          lod.tgi,
        );
        if (token !== generation) return;
        let source: BuildingMaterialSource;
        try {
          source = await analyzeBuildingMaterial(
            parseLotModelContainer(buffer),
          );
        } catch {
          if (token === generation) errorKey.value = "propertyUnsupported";
          return;
        }
        if (token !== generation) return;
        resolved.value = {
          source,
          name: session.assetName ?? input.value.trim(),
          binding: {
            packageId: entry.package.packageId,
            packagePath: entry.package.path,
            propertyTgi: {
              type: tgi.typeId,
              group: tgi.group,
              instance: tgi.instance,
            },
            modelPackageId: lod.packageId,
            modelPackagePath: packages.opened.find(
              (item) => item.package.packageId === lod.packageId,
            )?.package.path,
            modelTgi: {
              type: lod.tgi.typeId,
              group: lod.tgi.group,
              instance: lod.tgi.instance,
            },
            lod: lodIndex + 1,
          },
        };
        return;
      }
      errorKey.value = "propertyNotFound";
    } catch {
      if (token === generation) errorKey.value = "propertyResolveFailed";
    } finally {
      if (token === generation) loading.value = false;
    }
  }

  watch(
    [input, packageIds],
    () => {
      // Invalidate immediately, not after debounce: old requests may finish while typing.
      ++generation;
      clearTimeout(timer);
      resolved.value = null;
      errorKey.value = "";
      loading.value = !!input.value.trim();
      timer = setTimeout(() => void resolve(), 450);
    },
    { flush: "sync" },
  );
  onScopeDispose(() => {
    ++generation;
    clearTimeout(timer);
  });
  return { input, loading, errorKey, resolved, resolve };
}
