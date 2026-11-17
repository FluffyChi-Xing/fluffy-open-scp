<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import ProjectConfigTabs from "../asset/ProjectConfigTabs.vue";
import RoadProjectMenu from "./RoadProjectMenu.vue";
import { recipeDraft, type RecipeId, type RoadSchema } from "./roadSchema";

const props = defineProps<{
  project: string;
  manifest: Record<string, unknown>;
  schema: RoadSchema;
  recipe?: RecipeId;
  metadata: Record<"author" | "version" | "description", string>;
  saving: boolean;
  dirty: boolean;
}>();
const emit = defineEmits<{
  close: [];
  metadata: [field: keyof typeof props.metadata, value: string];
  menu: [patch: Partial<Pick<RoadSchema, "title" | "description" | "menu">>];
}>();
const { t } = useI18n();
const icons = {
  basic: "FileText",
  menu: "Menu",
  road: "Box",
  dependencies: "Network",
  advanced: "Braces",
} as const;
const tabs = (Object.keys(icons) as (keyof typeof icons)[]).map((id) => ({
  id,
  icon: icons[id],
  labelKey: `roadAsset.project.tabs.${id}`,
}));
const tab = shallowRef<keyof typeof icons>("basic");
const strips = computed(
  () => props.schema.section ?? recipeDraft(props.schema.recipe),
);
const metrics = computed(() => ({
  width: Number(
    strips.value.reduce((sum, strip) => sum + strip.width, 0).toFixed(2),
  ),
  forward: strips.value.filter((strip) => strip.kind === "forward").length,
  backward: strips.value.filter((strip) => strip.kind === "backward").length,
  dividers: strips.value.filter((strip) => strip.kind === "median").length,
}));
const schemaPath = computed(() => {
  const build = props.manifest.build as { schema?: string } | undefined;
  return build?.schema ?? "";
});
const inputs = computed(() => [
  { role: "schema", path: schemaPath.value },
  { role: "game", path: props.schema.gamePackage },
  { role: "scripts", path: props.schema.scriptsPackage },
  { role: "menuIcon", path: props.schema.menu?.iconPng },
  { role: "menuMarquee", path: props.schema.menu?.marqueePng },
]);
interface ResourceReport {
  packageSha256: string;
  packages: string[];
  resources: { tgi: string; mode: "bundled" | "external"; package?: string }[];
  unresolved: { from: string; field?: string; reference: string }[];
}
const report = computed(() => {
  const dependencies = props.manifest.dependencies as
    { road?: { resolved?: ResourceReport } } | undefined;
  return props.dirty ? undefined : dependencies?.road?.resolved;
});
</script>

<template>
  <section class="project-panel" :aria-label="t('roadAsset.project.heading')">
    <header>
      <strong>{{ t("roadAsset.project.heading") }}</strong>
      <button type="button" @click="emit('close')">
        {{ t("roadAsset.back") }}
      </button>
    </header>
    <div class="body">
      <ProjectConfigTabs
        v-model="tab"
        :tabs="tabs"
        :label="t('roadAsset.project.heading')"
      />
      <div class="content">
        <template v-if="tab === 'basic'">
          <section class="card">
            <h3>{{ manifest.name ?? schema.title }}</h3>
            <label
              v-for="field in ['author', 'version', 'description'] as const"
              :key="field"
            >
              {{ t(`roadAsset.project.fields.${field}`) }}
              <textarea
                v-if="field === 'description'"
                :value="metadata[field]"
                :disabled="saving"
                rows="3"
                @input="
                  emit(
                    'metadata',
                    field,
                    ($event.target as HTMLTextAreaElement).value,
                  )
                "
              />
              <input
                v-else
                :value="metadata[field]"
                :disabled="saving"
                @input="
                  emit(
                    'metadata',
                    field,
                    ($event.target as HTMLInputElement).value,
                  )
                "
              />
            </label>
          </section>
          <section class="card">
            <h3>{{ t("roadAsset.project.build") }}</h3>
            <dl>
              <div>
                <dt>{{ t("roadAsset.project.identity") }}</dt>
                <dd>
                  <code>{{
                    schema.identity.toString(16).toUpperCase().padStart(8, "0")
                  }}</code>
                </dd>
              </div>
              <div>
                <dt>{{ t("roadAsset.project.inputs.schema") }}</dt>
                <dd>
                  <code>{{ schemaPath }}</code>
                </dd>
              </div>
              <div>
                <dt>{{ t("roadAsset.project.template") }}</dt>
                <dd>
                  {{
                    recipe
                      ? t(`roadAsset.recipes.${recipe}`)
                      : t("roadAsset.project.custom")
                  }}
                </dd>
              </div>
            </dl>
          </section>
        </template>
        <RoadProjectMenu
          v-else-if="tab === 'menu'"
          :project="project"
          :schema="schema"
          :saving="saving"
          @update="emit('menu', $event)"
        />
        <template v-else-if="tab === 'road'">
          <div class="metrics">
            <section class="card">
              <h3>{{ t("roadAsset.crossSection") }}</h3>
              <strong>{{
                t("roadAsset.draft.totalWidth", { width: metrics.width })
              }}</strong>
              <span>{{
                t("roadAsset.project.stripCount", { count: strips.length })
              }}</span>
            </section>
            <section class="card">
              <h3>{{ t("roadAsset.project.traffic") }}</h3>
              <span>{{
                t("roadAsset.project.forwardCount", { count: metrics.forward })
              }}</span>
              <span>{{
                t("roadAsset.project.backwardCount", {
                  count: metrics.backward,
                })
              }}</span>
            </section>
            <section class="card">
              <h3>{{ t("roadAsset.project.features") }}</h3>
              <span>{{
                t("roadAsset.project.dividerCount", { count: metrics.dividers })
              }}</span>
              <span
                v-if="
                  schema.composition?.leftTrees ||
                  schema.composition?.rightTrees
                "
              >
                {{
                  [
                    schema.composition.leftTrees &&
                      t("roadAsset.components.leftTrees"),
                    schema.composition.rightTrees &&
                      t("roadAsset.components.rightTrees"),
                  ]
                    .filter(Boolean)
                    .join(" · ")
                }}
              </span>
              <span v-if="schema.composition?.tram">{{
                t("roadAsset.components.tram")
              }}</span>
            </section>
          </div>
          <section class="card">
            <h3>{{ t("roadAsset.crossSection") }}</h3>
            <ol v-if="strips.length">
              <li v-for="strip in strips" :key="strip.id">
                <span>{{ t(`roadAsset.strips.${strip.kind}`) }}</span>
                <span>{{ t("roadAsset.metres", { width: strip.width }) }}</span>
              </li>
            </ol>
            <p v-else>{{ t("roadAsset.draft.previewEmpty") }}</p>
          </section>
        </template>
        <template v-else-if="tab === 'dependencies'">
          <section class="card">
            <h3>{{ t("roadAsset.project.inputFiles") }}</h3>
            <dl>
              <div v-for="input in inputs" :key="input.role">
                <dt>{{ t(`roadAsset.project.inputs.${input.role}`) }}</dt>
                <dd>
                  <code v-if="input.path">{{ input.path }}</code
                  ><span v-else>{{ t("roadAsset.project.unset") }}</span>
                </dd>
              </div>
            </dl>
          </section>
          <section class="card">
            <h3>{{ t("roadAsset.project.buildDependencies") }}</h3>
            <template v-if="report">
              <code>{{ report.packageSha256 }}</code>
              <dl>
                <div v-for="path in report.packages" :key="path">
                  <dd>
                    <code>{{ path }}</code>
                  </dd>
                </div>
              </dl>
              <details>
                <summary>
                  {{
                    t("roadAsset.project.resourceCount", {
                      count: report.resources.length,
                    })
                  }}
                </summary>
                <ul>
                  <li v-for="item in report.resources" :key="item.tgi">
                    <code>{{ item.tgi }}</code
                    ><span>{{
                      item.package ?? t("roadAsset.project.bundled")
                    }}</span>
                  </li>
                </ul>
              </details>
              <details v-if="report.unresolved.length">
                <summary>
                  {{
                    t("roadAsset.project.unresolvedCount", {
                      count: report.unresolved.length,
                    })
                  }}
                </summary>
                <ul>
                  <li
                    v-for="item in report.unresolved"
                    :key="`${item.from}:${item.field}:${item.reference}`"
                  >
                    <code>{{
                      [item.from, item.field, item.reference]
                        .filter(Boolean)
                        .join(" → ")
                    }}</code>
                  </li>
                </ul>
              </details>
            </template>
            <p v-else>{{ t("roadAsset.project.dependenciesPending") }}</p>
          </section>
        </template>
        <template v-else>
          <p>{{ t("roadAsset.project.savedJson") }}</p>
          <pre>{{ JSON.stringify(manifest, null, 2) }}</pre>
        </template>
      </div>
    </div>
  </section>
</template>

<style scoped>
.project-panel {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  font-size: 12px;
}
header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
}
.body {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  flex: 1;
  min-height: 0;
}
.content {
  overflow: auto;
  min-width: 0;
  padding: 12px;
}
.content > * + * {
  margin-block-start: 10px;
}
.card {
  display: grid;
  gap: 8px;
  min-width: 0;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 12px;
}
.metrics {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 138px), 1fr));
  gap: 8px;
}
label {
  display: grid;
  gap: 4px;
}
button,
input,
textarea {
  background: var(--surface);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 5px 8px;
}
textarea {
  resize: vertical;
}
h3 {
  font-weight: 600;
}
dl,
ul,
ol,
p {
  margin: 0;
}
dl > div,
li {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 4px 10px;
}
dd {
  margin: 0;
}
dt,
p,
code {
  color: var(--muted-foreground);
  overflow-wrap: anywhere;
}
ol,
ul {
  display: grid;
  gap: 5px;
  padding: 0;
}
ol li {
  justify-content: space-between;
}
pre {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
