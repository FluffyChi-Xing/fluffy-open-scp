<script setup lang="ts">
import { ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { command, type Tgi } from "@/api/tauri";
import FSpinner from "@/components/ui/FSpinner.vue";
import DecalDictionaryGallery from "@/pages/packages/components/preview/DecalDictionaryGallery.vue";
import { useGamePackagesStore } from "@/stores/gamePackages";
import type { DecalSelection } from "./workbenchDecals";

defineProps<{ editEnabled?: boolean }>();
const emit = defineEmits<{ place: [selection: DecalSelection] }>();

interface DecalPackage {
  packageId: number;
  path: string;
  atlases: { tgi: Tgi; entryCount: number }[];
}

const { t } = useI18n();
const packages = useGamePackagesStore();
const catalog = shallowRef<DecalPackage[]>([]);
const expanded = ref(new Set<number>());
const loading = ref(false);
const failed = ref(false);

watch(
  () => packages.opened.map((item) => item.package.packageId).join(","),
  async (_ids, _previous, onCleanup) => {
    let active = true;
    onCleanup(() => { active = false; });
    loading.value = true;
    failed.value = false;
    try {
      const groups = await command<DecalPackage[]>("list_decal_library");
      if (active) catalog.value = groups;
    } catch {
      if (active) {
        catalog.value = [];
        failed.value = true;
      }
    } finally {
      if (active) loading.value = false;
    }
  },
  { immediate: true },
);

function updateExpanded(packageId: number, event: Event): void {
  const details = event.currentTarget as HTMLDetailsElement;
  if (details.open) expanded.value.add(packageId);
  else expanded.value.delete(packageId);
}

function atlasLabel(tgi: Tgi): string {
  return [tgi.typeId, tgi.group, tgi.instance]
    .map((part) => part.toString(16).padStart(8, "0").toUpperCase())
    .join(":");
}
</script>

<template>
  <div class="decal-library">
    <p v-if="loading" class="decal-library-status">
      <FSpinner size="sm" :label="t('common.loading')" />
    </p>
    <p v-else-if="failed" class="decal-library-status" role="alert">
      {{ t("studio.assetWorkbench.decalLibraryError") }}
    </p>
    <p v-else-if="!catalog.length" class="decal-library-status">
      {{ t("studio.assetWorkbench.decalLibraryEmpty") }}
    </p>
    <details
      v-for="group in catalog"
      :key="group.packageId"
      class="decal-library-package"
      :open="expanded.has(group.packageId)"
      @toggle="updateExpanded(group.packageId, $event)"
    >
      <summary :title="group.path">
        {{ group.path.split(/[\\/]/).at(-1) }}
        <span class="decal-library-count">
          {{ group.atlases.reduce((total, atlas) => total + atlas.entryCount, 0) }}
        </span>
      </summary>
      <template v-if="expanded.has(group.packageId)">
        <section v-for="atlas in group.atlases" :key="atlasLabel(atlas.tgi)" class="decal-library-atlas">
          <h3>{{ atlasLabel(atlas.tgi) }}</h3>
          <DecalDictionaryGallery
            :package-id="group.packageId"
            :tgi="atlas.tgi"
            :placeable="editEnabled"
            compact
            @place="entry => entry.id && emit('place', {
              id: entry.id,
              source: { packagePath: group.path, atlas: atlas.tgi },
            })"
          />
        </section>
      </template>
    </details>
  </div>
</template>

<style scoped>
.decal-library {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.decal-library-status {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 8px;
  text-align: center;
}
.decal-library-package {
  margin-bottom: 7px;
}
.decal-library-package summary {
  background: var(--surface-elevated);
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: 11px;
  font-weight: 600;
  min-height: 27px;
  padding: 5px 8px;
}
.decal-library-count {
  background: var(--surface-hover);
  border-radius: 999px;
  color: var(--muted-foreground);
  float: right;
  font-size: 10px;
  padding: 0 7px;
}
.decal-library-atlas {
  margin-block: 6px 10px;
}
.decal-library-atlas h3 {
  color: var(--muted-foreground);
  font-family: monospace;
  font-size: 10px;
  margin: 0 2px 4px;
}
</style>
