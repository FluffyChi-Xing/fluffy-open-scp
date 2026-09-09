<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";

export type PanelStage = "resolve" | "edit" | "build" | "verify";

const props = withDefaults(
  defineProps<{
    panel: string;
    icon: string;
    stage: PanelStage;
    status: "next" | "planned" | "preview";
    to: string;
    wide?: boolean;
  }>(),
  { wide: false },
);

const { t } = useI18n();
</script>

<template>
  <RouterLink
    class="panel-card"
    :class="{ [`stage-${props.stage}`]: true, wide: props.wide }"
    :to="props.to"
  >
    <div class="panel-top">
      <span class="panel-icon"><FIcon :name="props.icon" :size="18" /></span>
      <span class="panel-status" :data-status="props.status">
        {{ t(`studio.panels.${props.panel}.status`) }}
      </span>
    </div>
    <FTypography class="panel-title" :header="4" spacing="none">
      {{ t(`studio.panels.${props.panel}.title`) }}
    </FTypography>
    <p class="panel-desc">{{ t(`studio.panels.${props.panel}.description`) }}</p>
    <p class="panel-meta">{{ t(`studio.panels.${props.panel}.meta`) }}</p>
  </RouterLink>
</template>

<style scoped>
.panel-card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 1.25rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--foreground);
  text-decoration: none;
  transition:
    border-color 160ms ease,
    transform 160ms ease;
  overflow: hidden;
}
.panel-card::before {
  content: "";
  position: absolute;
  inset: 0 auto 0 0;
  width: 2px;
  background: var(--stage-color, var(--border));
  opacity: 0.7;
}
.panel-card:hover {
  border-color: var(--stage-color, var(--border));
  transform: translateY(-2px);
}
.panel-card:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}
.stage-resolve {
  --stage-color: var(--info);
}
.stage-edit {
  --stage-color: var(--accent);
}
.stage-build {
  --stage-color: var(--warning);
}
.stage-verify {
  --stage-color: var(--brand);
}
.wide {
  grid-column: span 2;
}
.panel-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.panel-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--stage-color, var(--muted-foreground));
  background: color-mix(in oklab, var(--stage-color, var(--border)) 10%, transparent);
}
.panel-status {
  font-size: 0.6875rem;
  letter-spacing: 0.02em;
  padding: 0.15rem 0.55rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  font-family: var(--font-mono, ui-monospace, monospace);
}
.panel-status[data-status="next"] {
  color: var(--stage-color, var(--primary));
  border-color: color-mix(in oklab, var(--stage-color, var(--primary)) 45%, transparent);
}
.panel-title :deep(*) {
  margin: 0;
}
.panel-desc {
  margin: 0;
  font-size: 0.8125rem;
  line-height: 1.55;
  color: var(--muted-foreground);
}
.panel-meta {
  margin: 0;
  margin-top: auto;
  padding-top: 0.5rem;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
}
@media (prefers-reduced-motion: reduce) {
  .panel-card {
    transition: none;
  }
  .panel-card:hover {
    transform: none;
  }
}
</style>
