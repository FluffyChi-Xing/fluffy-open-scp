<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import { tauriApi } from "@/api";
import FIcon from "@/components/extensions/FIcon.vue";
import SceneWorkbenchLayout from "../SceneWorkbenchLayout.vue";
import AssetSourceTree from "../asset/AssetSourceTree.vue";
import RoadCrossSection from "./RoadCrossSection.vue";
import RoadPreview from "./RoadPreview.vue";
import RoadRecipeLibrary from "./RoadRecipeLibrary.vue";
import { parseRoadSchema, type RecipeId, type RoadSchema } from "./roadSchema";

const props = defineProps<{
  project: string;
  manifest: Record<string, unknown>;
}>();
const { t } = useI18n();
const sourceTree = ref<InstanceType<typeof AssetSourceTree> | null>(null);
const schema = ref<RoadSchema | null>(null);
const file = shallowRef<string | null>(null);
const revision = shallowRef<string>();
const baseline = shallowRef("");
const status = shallowRef("");
const saving = shallowRef(false);
const sourcePath = computed(() => {
  const build = props.manifest.build as { schema?: unknown } | undefined;
  return typeof build?.schema === "string" ? build.schema : "";
});
const dirty = computed(
  () => !!schema.value && JSON.stringify(schema.value) !== baseline.value,
);

watch(
  sourcePath,
  async (path, _, onCleanup) => {
    let cancelled = false;
    onCleanup(() => {
      cancelled = true;
    });
    schema.value = null;
    status.value = path ? "loading" : "invalid";
    if (!path) return;
    try {
      const document = await tauriApi.workspace.codeReadText(
        props.project,
        path,
      );
      const parsed = parseRoadSchema(document.content);
      if (cancelled) return;
      schema.value = parsed;
      baseline.value = JSON.stringify(parsed);
      revision.value = document.revision;
      status.value = "";
    } catch {
      if (!cancelled) status.value = "loadFailed";
    }
  },
  { immediate: true },
);

function select(recipe: RecipeId) {
  if (!schema.value || schema.value.recipe === recipe) return;
  schema.value.recipe = recipe;
  if (recipe !== "highStreet") delete schema.value.streetlights;
}

async function chooseSource(key: "gamePackage" | "scriptsPackage") {
  try {
    const path = await open({
      filters: [{ name: t("roadAsset.packageFile"), extensions: ["package"] }],
    });
    if (typeof path === "string" && schema.value) schema.value[key] = path;
  } catch {
    status.value = "loadFailed";
  }
}

async function save() {
  if (!schema.value || saving.value || !schema.value.title.trim()) return false;
  if (!dirty.value) return true;
  saving.value = true;
  const snapshot = JSON.stringify(schema.value);
  try {
    const saved = await tauriApi.workspace.codeWriteText(
      props.project,
      sourcePath.value,
      JSON.stringify(JSON.parse(snapshot), null, 2) + "\n",
      revision.value,
    );
    if (saved.relativePath !== sourcePath.value) return false;
    revision.value = saved.revision;
    baseline.value = snapshot;
    status.value = "saved";
    void sourceTree.value?.refresh();
    return !dirty.value;
  } catch {
    status.value = "saveFailed";
    return false;
  } finally {
    saving.value = false;
  }
}
defineExpose({ save });
</script>

<template>
  <section class="road-workbench" :aria-label="t('roadAsset.title')">
    <header class="toolbar">
      <FIcon name="Box" :size="14" aria-label="" />
      <strong>{{ t("roadAsset.title") }}</strong>
      <input
        v-if="schema"
        v-model="schema.title"
        :disabled="saving"
        :aria-label="t('roadAsset.name')"
      />
      <button
        v-if="schema"
        type="button"
        :disabled="!dirty || saving || !schema.title.trim()"
        @click="save"
      >
        {{ t(saving ? "roadAsset.saving" : "roadAsset.save") }}
      </button>
      <span v-if="status" role="status">{{ t(`roadAsset.${status}`) }}</span>
      <span v-else-if="dirty" role="status">{{ t("roadAsset.unsaved") }}</span>
    </header>
    <SceneWorkbenchLayout>
      <template #files>
        <div class="files">
          <AssetSourceTree
            ref="sourceTree"
            :project="project"
            :active-asset="sourcePath"
            :selected-path="file"
            read-only
            @select="file = $event"
          />
        </div>
      </template>
      <template v-if="schema && file === 'package.json'" #center>
        <div class="manifest">
          <button type="button" @click="file = null">
            {{ t("roadAsset.back") }}
          </button>
          <pre>{{ JSON.stringify(manifest, null, 2) }}</pre>
        </div>
      </template>
      <template #viewport>
        <RoadPreview
          v-if="schema"
          :project="project"
          :game-package="schema.gamePackage"
          :recipe="schema.recipe"
          :streetlights="schema.streetlights ?? 'original'"
        />
      </template>
      <template #lower>
        <RoadCrossSection
          v-if="schema"
          :recipe="schema.recipe"
          :streetlights="schema.streetlights ?? 'original'"
          @select="select"
        />
      </template>
      <template #library>
        <div v-if="schema" class="side">
          <div class="inputs">
            <button
              type="button"
              :title="schema.gamePackage"
              @click="chooseSource('gamePackage')"
            >
              {{
                t(
                  schema.gamePackage
                    ? "roadAsset.changeGame"
                    : "roadAsset.chooseGame",
                )
              }}
            </button>
            <button
              type="button"
              :title="schema.scriptsPackage"
              @click="chooseSource('scriptsPackage')"
            >
              {{
                t(
                  schema.scriptsPackage
                    ? "roadAsset.changeScripts"
                    : "roadAsset.chooseScripts",
                )
              }}
            </button>
          </div>
          <RoadRecipeLibrary
            :selected="schema.recipe"
            :streetlights="schema.streetlights ?? 'original'"
            @select="select"
            @lighting="schema.streetlights = $event"
          />
        </div>
      </template>
    </SceneWorkbenchLayout>
  </section>
</template>

<style scoped>
.road-workbench {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  min-width: 0;
  background: var(--surface);
}
.toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  padding: 7px 10px;
  border-bottom: 1px solid var(--border);
  font-size: 12px;
}
.toolbar span {
  color: var(--muted-foreground);
}
.toolbar input {
  flex: 1;
  min-width: 120px;
}
button,
input {
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--foreground);
  background: var(--surface-elevated);
  padding: 4px 8px;
}
button:disabled {
  opacity: 0.5;
}
.files,
.manifest {
  height: 100%;
  overflow: auto;
}
.manifest {
  padding: 12px;
}
.side {
  height: 100%;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
}
.inputs {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  padding: 8px 10px;
  font-size: 11px;
  border-bottom: 1px solid var(--border);
}
</style>
