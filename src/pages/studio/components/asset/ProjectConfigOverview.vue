<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import ConfigReadonlyValue from "./ConfigReadonlyValue.vue";
import type { LogicGraph } from "./logicGraph";
import { LOGIC_NODE_SPECS } from "./logicGraph";
import type { WorkbenchMenuBinding } from "./menuBinding";
import { menuTargetLabelKey } from "./menuBinding";
import { useMenuImages } from "./useMenuImages";
const props = defineProps<{
  manifest: Record<string, unknown>;
  menu: WorkbenchMenuBinding;
  logic: LogicGraph;
  variant: boolean;
  advancedText?: string;
  project?: string;
}>();
const { t } = useI18n();
const variantConfig = computed(
  () => props.manifest.vanillaVariant as Record<string, unknown> | undefined,
);
const basic = computed(() =>
  Object.fromEntries(
    [
      "name",
      "version",
      "description",
      "author",
      "mod_type",
      "workflow",
      "format",
      "manifest_version",
    ].map((key) => [key, props.manifest[key]]),
  ),
);
const building = computed(() =>
  props.variant
    ? {
        fields: variantConfig.value?.fields,
        powerProduction: variantConfig.value?.powerProduction,
      }
    : props.logic.nodes
        .filter((n) => n.kind === "building-info")
        .map((n) => props.logic.params[n.id]),
);
const behaviors = computed(() =>
  props.logic.nodes
    .filter((n) => n.kind !== "building-info")
    .map((n) => ({
      id: n.id,
      label: t(
        LOGIC_NODE_SPECS.find(
          (s) =>
            s.kind === n.kind &&
            s.leafType ===
              (props.logic.params[n.id] as { type?: string } | undefined)?.type,
        )?.labelKey ?? "studio.assetWorkbench.projectConfig.tabBehavior",
      ),
      params: props.logic.params[n.id],
    })),
);
const menu = computed(() =>
  props.variant
    ? Object.fromEntries(
        ["title", "description", "iconPng", "marqueePng", "unit", "lot"].map(
          (k) => [k, variantConfig.value?.[k]],
        ),
      )
    : {
        ...props.menu,
        entries: props.menu.entries.map((entry) => {
          const target = menuTargetLabelKey(
            entry.dimension,
            entry.categoryInstance,
          );
          return {
            ...entry,
            dimension: t(
              entry.dimension === "city"
                ? "studio.workbench.dimCity"
                : "studio.workbench.dimDevelopment",
            ),
            categoryInstance: target
              ? `${t(target)} (${entry.categoryInstance})`
              : entry.categoryInstance,
          };
        }),
      },
);
const { previews } = useMenuImages(
  () => props.project,
  () => ({
    iconPng: String(menu.value.iconPng ?? ""),
    marqueePng: String(menu.value.marqueePng ?? ""),
  }),
  () => {},
);
const advanced = computed(
  () => props.advancedText ?? JSON.stringify(props.manifest, null, 2),
);
</script>
<template>
  <div class="config-overview">
    <details open>
      <summary>{{ t("studio.assetWorkbench.projectConfig.tabBasic") }}</summary>
      <ConfigReadonlyValue :value="basic" />
    </details>
    <details open>
      <summary>{{ t("studio.assetWorkbench.projectConfig.tabMenu") }}</summary>
      <div v-if="previews.icon || previews.marquee" class="overview-images">
        <figure v-for="kind in ['icon', 'marquee'] as const" :key="kind">
          <template v-if="previews[kind]">
            <img
              :src="previews[kind]"
              :alt="
                t(
                  kind === 'icon'
                    ? 'studio.assetWorkbench.menuIcon'
                    : 'studio.assetWorkbench.menuMarquee',
                )
              "
            />
            <figcaption>
              {{
                t(
                  kind === "icon"
                    ? "studio.assetWorkbench.menuIcon"
                    : "studio.assetWorkbench.menuMarquee",
                )
              }}
            </figcaption>
          </template>
        </figure>
      </div>
      <ConfigReadonlyValue :value="menu" />
    </details>
    <details open>
      <summary>
        {{ t("studio.assetWorkbench.projectConfig.tabBuilding") }}
      </summary>
      <ConfigReadonlyValue :value="building" />
    </details>
    <details v-if="!variant" open>
      <summary>
        {{ t("studio.assetWorkbench.projectConfig.tabBehavior") }}
      </summary>
      <section
        v-for="behavior in behaviors"
        :key="behavior.id"
        class="overview-behavior"
      >
        <h3>{{ behavior.label }}</h3>
        <ConfigReadonlyValue :value="behavior.params" />
      </section>
      <p v-if="!behaviors.length">
        {{ t("studio.assetWorkbench.overviewEmpty") }}
      </p>
    </details>
    <details>
      <summary>
        {{ t("studio.assetWorkbench.projectConfig.tabDependencies") }}
      </summary>
      <ConfigReadonlyValue
        :value="{
          nodes: manifest.nodes,
          internal_dependencies: manifest.internal_dependencies,
          dependencies: manifest.dependencies,
        }"
      />
    </details>
    <details>
      <summary>
        {{ t("studio.assetWorkbench.projectConfig.tabAdvanced") }}
      </summary>
      <pre>{{ advanced }}</pre>
    </details>
  </div>
</template>
<style scoped>
.config-overview {
  display: grid;
  gap: 10px;
}
.overview-images {
  display: flex;
  gap: 20px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.overview-images figure {
  margin: 0;
}
.overview-images img {
  height: 80px;
  max-width: 180px;
  object-fit: contain;
}
.overview-images figcaption {
  font-size: 11px;
  color: var(--muted-foreground);
  margin-top: 4px;
}
details {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 12px;
  min-width: 0;
}
summary {
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
  color: var(--foreground);
  padding: 2px 0;
}
summary:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 4px;
}
details[open] > summary {
  margin-bottom: 12px;
}
.overview-behavior + .overview-behavior {
  border-top: 1px solid var(--border);
  margin-top: 12px;
  padding-top: 12px;
}
h3 {
  font-size: 12px;
  margin: 0 0 10px;
}
p {
  font-size: 12px;
  color: var(--muted-foreground);
}
pre {
  margin: 0;
  font-size: 11px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-width: 100%;
}
</style>
