<script setup lang="ts">
import { Switch } from "@/components/ui/switch";
import { computed, ref, shallowRef, useId, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import { command } from "@/api/tauri";
import FIcon from "@/components/extensions/FIcon.vue";
import FButton from "@/components/ui/FButton.vue";
import FSelect from "@/components/ui/FSelect.vue";
import SceneWorkbenchLayout from "../SceneWorkbenchLayout.vue";
import AssetSourceTree from "../asset/AssetSourceTree.vue";
import RoadCrossSection from "./RoadCrossSection.vue";
import RoadComponentPalette from "./RoadComponentPalette.vue";
import RoadDraftSection from "./RoadDraftSection.vue";
import RoadPreview from "./RoadPreview.vue";
import RoadProjectPanel from "./RoadProjectPanel.vue";
import RoadRecipeLibrary from "./RoadRecipeLibrary.vue";
import RoadStripLibrary from "./RoadStripLibrary.vue";
import {
  matchingRecipe,
  draftStreetlightOptions,
  parseRoadSchema,
  pavedRecipes,
  recipes,
  type PavedRecipe,
  type RecipeId,
  type RoadSchema,
  type Streetlights,
} from "./roadSchema";
import { availableComponents, type RoadComponent } from "./roadComposition";
import {
  insertStrip,
  stripWidths,
  type RoadStrip,
  type StripKind,
} from "./roadDraft";

const props = defineProps<{
  project: string;
  manifest: Record<string, unknown>;
}>();
const { t } = useI18n();
const baseRecipeId = useId();
const streetlightsId = useId();
const lightingOptions = computed(() =>
  draftStreetlightOptions.map((value) => ({
    value,
    label: t(value === "original" ? "roadAsset.lighting.default" : `roadAsset.lighting.${value}`),
  })),
);
const baseOptions = computed(() =>
  pavedRecipes.map((recipe) => ({
    value: recipe,
    label: t(`roadAsset.recipes.${recipe}`),
  })),
);
const sourceTree = ref<InstanceType<typeof AssetSourceTree> | null>(null);
const schema = ref<RoadSchema | null>(null);
const file = shallowRef<string | null>(null);
interface RoadProjectState {
  schema: RoadSchema;
  manifest: Record<string, unknown>;
  manifestRevision: string;
  schemaRevision: string;
}
const state = shallowRef<RoadProjectState>();
const metadata = ref({ author: "", version: "", description: "" });
const baseline = shallowRef("");
const status = shallowRef("");
const saving = shallowRef(false);
const sourcePath = computed(() => {
  const build = (state.value?.manifest ?? props.manifest).build as
    { schema?: unknown } | undefined;
  return typeof build?.schema === "string" ? build.schema : "";
});
const snapshot = computed(() =>
  JSON.stringify({ schema: schema.value, metadata: metadata.value }),
);
const dirty = computed(
  () => !!schema.value && snapshot.value !== baseline.value,
);
const needsManifestSync = computed(() => {
  const dependencies = state.value?.manifest.dependencies as
    { road?: unknown } | undefined;
  return !dependencies?.road;
});
const draft = computed(() => schema.value?.section);
const baseRecipe = computed(() => schema.value?.baseRecipe);
const widths = computed(() => {
  if (!baseRecipe.value) return stripWidths;
  const recipe = recipes[baseRecipe.value];
  return {
    ...stripWidths,
    forward: recipe.width,
    backward: recipe.width,
    walk: recipe.walkWidth,
  };
});
const nativeRecipe = computed(() => {
  if (!schema.value) return undefined;
  if (!draft.value) return schema.value.recipe;
  if (schema.value.streetlights && schema.value.streetlights !== "original") return undefined;
  const match = matchingRecipe(draft.value);
  return match === baseRecipe.value ? match : undefined;
});
const previewRecipe = computed(() =>
  draft.value ? baseRecipe.value : nativeRecipe.value,
);

function projectAction(action: string, payload = {}) {
  return command<RoadProjectState>("code_flow", {
    request: { project: props.project, action, payload },
  });
}

watch(
  () => props.project,
  async (_, __, onCleanup) => {
    let cancelled = false;
    onCleanup(() => {
      cancelled = true;
    });
    schema.value = null;
    state.value = undefined;
    status.value = "loading";
    try {
      const project = await projectAction("road-project-read");
      const parsed = parseRoadSchema(JSON.stringify(project.schema));
      if (cancelled) return;
      state.value = project;
      schema.value = parsed;
      metadata.value = {
        author: String(project.manifest.author ?? ""),
        version: String(project.manifest.version ?? ""),
        description: String(project.manifest.description ?? ""),
      };
      baseline.value = snapshot.value;
      status.value = "";
    } catch {
      if (!cancelled) status.value = "loadFailed";
    }
  },
  { immediate: true },
);

watch(file, async (path, _, onCleanup) => {
  if (path !== "package.json" || !state.value) return;
  let current = true;
  onCleanup(() => {
    current = false;
  });
  try {
    const refreshed = await projectAction("road-project-read");
    if (
      current &&
      refreshed.schemaRevision === state.value?.schemaRevision &&
      refreshed.manifestRevision === state.value.manifestRevision
    )
      state.value = refreshed;
  } catch {
    // The editor can still show its last saved manifest.
  }
});

function select(recipe: RecipeId) {
  if (!schema.value || recipe === "dirt" || schema.value.recipe === recipe)
    return;
  schema.value.recipe = recipe;
  if (recipe !== "highStreet") delete schema.value.streetlights;
  if (schema.value.composition) {
    const next = Object.fromEntries(
      availableComponents(recipe)
        .filter((component) => schema.value?.composition?.[component])
        .map((component) => [component, true]),
    );
    if (Object.keys(next).length) schema.value.composition = next;
    else delete schema.value.composition;
  }
}

function updateDraft(strips: RoadStrip[]) {
  if (!schema.value) return;
  schema.value.section = strips;
}

function startDraft() {
  if (!schema.value) return;
  delete schema.value.streetlights;
  delete schema.value.composition;
  updateDraft([]);
}

function chooseBase(value: string) {
  const recipe = value as PavedRecipe;
  if (schema.value && pavedRecipes.includes(recipe)) {
    schema.value.baseRecipe = recipe;
    if (recipe === "streetcar") delete schema.value.streetlights;
  }
}

function chooseLighting(value: string) {
  const next = value as Streetlights;
  if (schema.value && draftStreetlightOptions.includes(next)) schema.value.streetlights = next;
}

function addStrip(kind: StripKind) {
  if (draft.value && baseRecipe.value)
    updateDraft(
      insertStrip(draft.value, draft.value.length, kind, widths.value[kind]),
    );
}

function buildIssue(): string | undefined {
  if (!draft.value || nativeRecipe.value) return;
  if (!baseRecipe.value) return "roadAsset.draft.chooseBase";
  return draft.value.length ? undefined : "roadAsset.draft.emptyBuild";
}

function setComponent(component: RoadComponent, enabled: boolean) {
  if (
    !schema.value ||
    !availableComponents(schema.value.recipe).includes(component)
  )
    return;
  const composition = { ...schema.value.composition };
  if (enabled) composition[component] = true;
  else delete composition[component];
  if (Object.keys(composition).length > 0)
    schema.value.composition = composition;
  else delete schema.value.composition;
}

function toggleComponent(component: RoadComponent) {
  setComponent(component, !schema.value?.composition?.[component]);
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
  if (
    !schema.value ||
    !state.value ||
    saving.value ||
    !schema.value.title.trim()
  )
    return false;
  if (!dirty.value && !needsManifestSync.value) return true;
  saving.value = true;
  const savedSnapshot = snapshot.value;
  const project = props.project;
  try {
    const saved = await projectAction("road-project-save", {
      ...JSON.parse(savedSnapshot),
      manifestRevision: state.value.manifestRevision,
      schemaRevision: state.value.schemaRevision,
    });
    if (project !== props.project) return false;
    state.value = saved;
    baseline.value = savedSnapshot;
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
defineExpose({ save, buildIssue });
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
      <FButton
        v-if="schema"
        variant="primary"
        size="sm"
        :disabled="
          (!dirty && !needsManifestSync) || saving || !schema.title.trim()
        "
        @click="save"
      >
        {{ t(saving ? "roadAsset.saving" : "roadAsset.save") }}
      </FButton>
      <span v-if="status && !(status === 'saved' && dirty)" role="status">
        {{ t(`roadAsset.${status}`) }}
      </span>
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
      <template v-if="schema && state && file === 'package.json'" #center>
        <RoadProjectPanel
          :project="project"
          :schema="schema"
          :manifest="state.manifest"
          :metadata="metadata"
          :recipe="previewRecipe"
          :dirty="dirty"
          :saving="saving"
          @close="file = null"
          @metadata="(field, value) => (metadata[field] = value)"
          @menu="schema = { ...schema, ...$event }"
        />
      </template>
      <template #viewport>
        <RoadPreview
          v-if="schema && previewRecipe && (!draft || draft.length)"
          :project="project"
          :game-package="schema.gamePackage"
          :recipe="previewRecipe"
          :draft="nativeRecipe ? undefined : draft"
          :streetlights="schema.streetlights ?? 'original'"
          :composition="draft ? undefined : schema.composition"
        />
        <div v-else-if="draft" class="draft-hint" role="status">
          {{
            t(
              baseRecipe
                ? "roadAsset.draft.previewEmpty"
                : "roadAsset.draft.chooseBase",
            )
          }}
        </div>
      </template>
      <template #lower>
        <RoadDraftSection
          v-if="draft && baseRecipe"
          :strips="draft"
          :widths="widths"
          @update="updateDraft"
        />
        <div v-else-if="draft" class="draft-hint" role="status">
          {{ t("roadAsset.draft.chooseBase") }}
        </div>
        <RoadCrossSection
          v-else-if="schema"
          :recipe="schema.recipe"
          :streetlights="schema.streetlights ?? 'original'"
          :composition="schema.composition"
          @select="select"
          @add-component="setComponent($event, true)"
          @toggle-component="toggleComponent"
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
          <div class="road-library">
            <div class="road-options">
              <div v-if="draft" class="base-recipe">
                <label :for="baseRecipeId">{{
                  t("roadAsset.draft.baseRecipe")
                }}</label>
                <FSelect
                  :id="baseRecipeId"
                  :model-value="baseRecipe ?? t('roadAsset.draft.chooseBase')"
                  :options="baseOptions"
                  @update:model-value="chooseBase"
                />
              </div>
              <label class="frontage-option">
                {{ t("roadAsset.frontage.enabled") }}
                <Switch
                  :model-value="schema.frontageEnabled ?? true"
                  :disabled="saving"
                  :aria-label="t('roadAsset.frontage.enabled')"
                  @update:model-value="schema.frontageEnabled = $event"
                />
              </label>
              <div v-if="draft && baseRecipe && baseRecipe !== 'streetcar'" class="base-recipe">
                <label :for="streetlightsId">{{ t("roadAsset.streetlights") }}</label>
                <FSelect
                  :id="streetlightsId"
                  :model-value="schema.streetlights ?? 'original'"
                  :options="lightingOptions"
                  @update:model-value="chooseLighting"
                />
              </div>
            </div>
            <template v-if="draft">
              <RoadStripLibrary
                v-if="baseRecipe"
                :widths="widths"
                @add="addStrip"
              />
              <p
                v-if="baseRecipe && draft.length && !nativeRecipe"
                class="build-note"
              >
                {{ t("roadAsset.draft.authoredBuildHint") }}
              </p>
            </template>
            <template v-else>
              <div>
                <button type="button" class="start-draft" @click="startDraft">
                  {{ t("roadAsset.draft.startEmpty") }}
                </button>
                <RoadComponentPalette
                  :recipe="schema.recipe"
                  :composition="schema.composition"
                  @toggle="toggleComponent"
                />
              </div>
              <RoadRecipeLibrary
                :selected="schema.recipe"
                :options="pavedRecipes"
                :streetlights="schema.streetlights ?? 'original'"
                @select="select"
                @lighting="schema.streetlights = $event"
              />
            </template>
          </div>
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
.files {
  height: 100%;
  overflow: auto;
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
.road-library {
  min-height: 0;
  overflow: auto;
}
.road-options {
  display: grid;
  gap: 8px;
  padding: 10px 10px 4px;
  font-size: 12px;
}
.frontage-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-block-size: 28px;
  cursor: pointer;
}
.build-note {
  margin: 8px 10px;
  font-size: 12px;
}
.base-recipe {
  display: grid;
  gap: 6px;
}
.draft-hint {
  display: grid;
  place-items: center;
  block-size: 100%;
  padding: 12px;
  color: var(--muted-foreground);
  font-size: 12px;
  text-align: center;
}
.build-note {
  color: var(--muted-foreground);
}
.start-draft {
  margin: 8px 10px;
  font-size: 12px;
}
</style>
