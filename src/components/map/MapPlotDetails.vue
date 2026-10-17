<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FEmpty from "@/components/extensions/FEmpty.vue";
import type { Region3DData, Region3DPlot } from "@/lib/region-map";
import { cityResourceSummary } from "./map-state-fields";

const props = defineProps<{
  data: Region3DData | null;
  plot: Region3DPlot | null;
}>();
const { t, locale } = useI18n();
const name = computed(() => {
  const p = props.plot;
  if (!p) return "";
  if (p.kind === "greatwork") return t("studio.map.greatWorkSite");
  return (
    (locale.value.startsWith("zh") ? p.name : p.nameEn) ??
    t("studio.map.citySite", { id: p.uid })
  );
});
const resources = computed(() =>
  cityResourceSummary(props.data, props.plot?.uid),
);
const description = computed(() =>
  locale.value.startsWith("zh")
    ? props.plot?.info?.description
    : props.plot?.info?.descriptionEn,
);
const traits = computed(() => {
  const info = props.plot?.info;
  if (!info) return [];
  return [
    ["cityBuildable", info.buildable === null ? null : `${info.buildable}%`],
    [
      "cityWind",
      info.wind === null
        ? null
        : t("studio.map.introLevel", { value: info.wind }),
    ],
    [
      "cityRail",
      info.rail === null ? null : t(`studio.map.${info.rail ? "yes" : "no"}`),
    ],
    [
      "cityShipping",
      info.shipping === null
        ? null
        : t(`studio.map.${info.shipping ? "yes" : "no"}`),
    ],
  ].filter((row): row is [string, string] => row[1] !== null);
});
const entry = computed(() =>
  props.data?.staticLayers?.cities?.find((c) => c.site === props.plot?.uid),
);
</script>

<template>
  <section class="plot-details" aria-live="polite">
    <FEmpty
      v-if="!plot"
      icon-name="MapPin"
      variant="compact"
      :title="t('studio.map.emptyPlotTitle')"
      :desc="t('studio.map.emptyPlotDescription')"
    />
    <template v-else>
      <h3>{{ name }}</h3>
      <p class="subtitle">
        {{
          plot.kind === "city"
            ? t("studio.map.citySite", { id: plot.uid })
            : t("studio.map.greatWorkSite")
        }}
      </p>
      <p v-if="plot.kind === 'city'" class="description">
        {{ description ?? t("studio.map.descriptionUnavailable") }}
      </p>
      <dl class="properties">
        <template v-for="[label, value] in traits" :key="label">
          <dt>{{ t(`studio.map.${label}`) }}</dt>
          <dd>{{ value }}</dd>
        </template>
        <dt>{{ t("studio.map.cityPosition") }}</dt>
        <dd>{{ plot.x.toFixed(0) }}, {{ plot.y.toFixed(0) }} m</dd>
        <dt>{{ t("studio.map.cityElevation") }}</dt>
        <dd>{{ plot.z.toFixed(1) }} m</dd>
        <template v-if="plot.kind === 'city'">
          <dt>{{ t("studio.map.cityArea") }}</dt>
          <dd>2,048 × 2,048 m</dd>
          <dt>{{ t("studio.map.cityEnvironment") }}</dt>
          <dd>{{ entry?.environment ?? t("studio.map.noCityResources") }}</dd>
        </template>
      </dl>
      <template v-if="plot.kind === 'city'">
        <h4>{{ t("studio.map.cityResources") }}</h4>
        <p v-if="plot.info" class="note">
          {{
            t("studio.map.introResourceLevels", {
              coal: plot.info.coal ?? "—",
              oil: plot.info.oil ?? "—",
              ore: plot.info.ore ?? "—",
              water: plot.info.water ?? "—",
            })
          }}
        </p>
        <p class="note">{{ t("studio.map.cityResourceExplanation") }}</p>
        <dl class="resources">
          <template v-for="resource in resources" :key="resource.kind">
            <dt>
              {{
                t(
                  `studio.map.res${resource.kind[0]!.toUpperCase() + resource.kind.slice(1)}`,
                )
              }}
            </dt>
            <dd>
              {{
                resource.coverage === null
                  ? t("studio.map.resourceUnknown")
                  : t("studio.map.resourceCoverage", {
                      value: resource.coverage.toFixed(1),
                    })
              }}
            </dd>
          </template>
        </dl>
      </template>
    </template>
  </section>
</template>

<style scoped>
.plot-details {
  padding: 1rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
}
h3 {
  margin: 0;
  font-size: 1rem;
}
h4 {
  margin: 1.3rem 0 0.5rem;
  font-size: 0.8125rem;
}
.description {
  font-size: 0.8125rem;
  line-height: 1.7;
  margin: 1rem 0;
  white-space: pre-line;
}
.subtitle,
.note {
  color: var(--muted-foreground);
  font-size: 0.75rem;
  line-height: 1.6;
}
.properties,
.resources {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 0.75rem;
  font-size: 0.75rem;
  margin: 1rem 0 0;
}
dt {
  color: var(--muted-foreground);
}
dd {
  text-align: end;
  margin: 0;
  overflow-wrap: anywhere;
  font-variant-numeric: tabular-nums;
}
</style>
