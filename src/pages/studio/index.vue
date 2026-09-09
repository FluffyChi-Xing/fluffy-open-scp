<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import StudioPanelCard from "./components/StudioPanelCard.vue";

const { t } = useI18n();

const stages = ["resolve", "edit", "build", "verify"] as const;

const panels = computed(() => [
  { key: "diagnostics", icon: "Shield", stage: "verify", status: "next", to: "/studio/diagnostics" },
  { key: "i18n", icon: "Globe", stage: "edit", status: "next", to: "/studio/i18n" },
  { key: "property", icon: "FilePen", stage: "edit", status: "planned", to: "/studio/property" },
  { key: "raster", icon: "Image", stage: "edit", status: "planned", to: "/studio/raster" },
  { key: "asset", icon: "Boxes", stage: "build", status: "planned", to: "/studio/asset" },
] as const);

const stagePanels = (stage: (typeof stages)[number]) =>
  panels.value.filter((panel) => panel.stage === stage);
</script>

<template>
  <section class="studio-page">
    <header class="studio-header">
      <div>
        <p class="eyebrow">{{ t("studio.eyebrow") }}</p>
        <FTypography :header="1" spacing="none">{{ t("studio.title") }}</FTypography>
        <FTypography paragraphy type="secondary">{{ t("studio.description") }}</FTypography>
      </div>
      <div class="studio-baseline">
        <span class="baseline-dot" aria-hidden="true"></span>
        <span>{{ t("studio.baseline") }}</span>
      </div>
    </header>

    <nav class="pipeline" :aria-label="t('studio.pipelineLabel')">
      <ol class="pipeline-rail">
        <li v-for="(stage, index) in stages" :key="stage" class="pipeline-node">
          <span class="node-index">{{ String(index + 1).padStart(2, "0") }}</span>
          <span class="node-label">{{ t(`studio.stages.${stage}`) }}</span>
          <span
            v-if="index < stages.length - 1"
            class="node-connector"
            aria-hidden="true"
          ></span>
        </li>
      </ol>
    </nav>

    <div class="stage-groups">
      <section
        v-for="stage in stages"
        :key="stage"
        class="stage-group"
        :aria-label="t(`studio.stages.${stage}`)"
      >
        <h2 class="stage-heading">
          {{ t(`studio.stages.${stage}`) }}
          <span class="stage-count">{{ stagePanels(stage).length }}</span>
        </h2>
        <div v-if="stagePanels(stage).length" class="stage-grid">
          <StudioPanelCard
            v-for="panel in stagePanels(stage)"
            :key="panel.key"
            :panel="panel.key"
            :icon="panel.icon"
            :stage="panel.stage"
            :status="panel.status"
            :to="panel.to"
            :wide="stage === 'verify' || stage === 'build'"
          />
        </div>
        <p v-else class="stage-empty">{{ t(`studio.stageEmpty.${stage}`) }}</p>
      </section>
    </div>

    <footer class="studio-footer">
      <FIcon name="GitBranch" :size="14" />
      <span>{{ t("studio.footer") }}</span>
    </footer>
  </section>
</template>

<style scoped>
.studio-page {
  display: flex;
  flex-direction: column;
  gap: 2rem;
  max-width: 1080px;
  margin: 0 auto;
  padding: 2.5rem 1.5rem 3rem;
}
.studio-header {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 1.5rem;
  flex-wrap: wrap;
}
.eyebrow {
  margin: 0 0 0.4rem;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  text-transform: uppercase;
  letter-spacing: 0.14em;
  color: var(--subtle-foreground);
}
.studio-header :deep(h1) {
  margin: 0;
}
.studio-baseline {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.85rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  font-size: 0.75rem;
  color: var(--muted-foreground);
  background: var(--surface);
}
.baseline-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--success, var(--primary));
  box-shadow: 0 0 0 3px color-mix(in oklab, var(--success, var(--primary)) 20%, transparent);
}

.pipeline-rail {
  display: flex;
  align-items: center;
  gap: 0;
  list-style: none;
  margin: 0;
  padding: 0.9rem 1.1rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  overflow-x: auto;
}
.pipeline-node {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  white-space: nowrap;
}
.node-index {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
}
.node-label {
  font-size: 0.8125rem;
  font-weight: 500;
}
.node-connector {
  width: clamp(2rem, 6vw, 5rem);
  height: 1px;
  margin: 0 1rem;
  background: linear-gradient(
    to right,
    var(--border),
    color-mix(in oklab, var(--accent) 55%, var(--border))
  );
}

.stage-groups {
  display: grid;
  gap: 1.75rem;
}
.stage-heading {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  margin: 0 0 0.75rem;
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--subtle-foreground);
}
.stage-count {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  color: var(--muted-foreground);
}
.stage-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: 0.875rem;
}
.stage-empty {
  margin: 0;
  padding: 0.85rem 1rem;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  font-size: 0.75rem;
  color: var(--subtle-foreground);
}
.stage-empty:lang(en) {
  font-style: normal;
}

.studio-footer {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding-top: 1.25rem;
  border-top: 1px solid var(--border);
  font-size: 0.75rem;
  color: var(--subtle-foreground);
}
@media (max-width: 640px) {
  .studio-page {
    padding: 1.75rem 1rem 2rem;
  }
}
</style>
