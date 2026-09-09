<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";

const props = defineProps<{
  panel: string;
  icon: string;
  status: "next" | "planned";
}>();

const { t } = useI18n();
const featureCount = 4;
</script>

<template>
  <section class="panel-stub">
    <RouterLink class="back-link" to="/studio">
      <FIcon name="ArrowLeft" :size="14" />
      {{ t("studio.backToStudio") }}
    </RouterLink>

    <header class="stub-header">
      <span class="stub-icon"><FIcon :name="props.icon" :size="20" /></span>
      <div>
        <FTypography :header="2" spacing="none">{{
          t(`studio.panels.${props.panel}.title`)
        }}</FTypography>
        <p class="stub-meta">{{ t(`studio.panels.${props.panel}.meta`) }}</p>
      </div>
      <span class="stub-status" :data-status="props.status">{{
        t(`studio.panels.${props.panel}.status`)
      }}</span>
    </header>

    <p class="stub-description">
      {{ t(`studio.panels.${props.panel}.description`) }}
    </p>

    <div class="stub-body">
      <section class="stub-section">
        <h2>{{ t("studio.stub.scopeTitle") }}</h2>
        <ul class="scope-list">
          <li v-for="index in featureCount" :key="index">
            {{ t(`studio.panels.${props.panel}.features.${index - 1}`) }}
          </li>
        </ul>
      </section>
      <section class="stub-section">
        <h2>{{ t("studio.stub.prereqTitle") }}</h2>
        <p>{{ t(`studio.panels.${props.panel}.prereq`) }}</p>
      </section>
    </div>
  </section>
</template>

<style scoped>
.panel-stub {
  padding-bottom: 3rem;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}
.back-link {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.75rem;
  color: var(--muted-foreground);
  text-decoration: none;
  width: fit-content;
}
.back-link:hover {
  color: var(--foreground);
}
.stub-header {
  display: flex;
  align-items: center;
  gap: 1rem;
}
.stub-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--accent);
  flex-shrink: 0;
}
.stub-header :deep(h2),
.stub-header :deep(h1) {
  margin: 0;
}
.stub-meta {
  margin: 0.2rem 0 0;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
}
.stub-status {
  margin-left: auto;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  padding: 0.2rem 0.6rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  white-space: nowrap;
}
.stub-status[data-status="next"] {
  color: var(--primary);
  border-color: color-mix(in oklab, var(--primary) 45%, transparent);
}
.stub-description {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 0.875rem;
  line-height: 1.6;
}
.stub-body {
  display: grid;
  gap: 0.875rem;
}
.stub-section {
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  padding: 1.1rem 1.25rem;
}
.stub-section h2 {
  margin: 0 0 0.6rem;
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--subtle-foreground);
}
.stub-section p {
  margin: 0;
  font-size: 0.8125rem;
  line-height: 1.6;
  color: var(--muted-foreground);
}
.scope-list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: grid;
  gap: 0.5rem;
}
.scope-list li {
  position: relative;
  padding-left: 1.25rem;
  font-size: 0.8125rem;
  line-height: 1.55;
}
.scope-list li::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0.55em;
  width: 5px;
  height: 5px;
  border-radius: 1px;
  background: var(--accent);
}
</style>
