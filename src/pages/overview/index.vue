<script setup lang="ts">
import { computed, onActivated } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import { useOpenScpOverview } from "@/composables/useOpenScpOverview";
import { usePackageStats } from "@/composables/usePackageStats";
import { useRenderTelemetry } from "@/composables/useRenderTelemetry";
import ExtensionStatsCards from "./components/ExtensionStatsCards.vue";
import RuntimeLogsCard from "./components/RuntimeLogsCard.vue";
import RenderStatsCard from "./components/RenderStatsCard.vue";

const overview = useOpenScpOverview();
const { snapshot, loading, error, demo } = overview;
let activatedOnce = false;
onActivated(() => {
  if (activatedOnce) void overview.reload();
  activatedOnce = true;
});
const historyPaths = computed(() =>
  Array.from(
    new Set((snapshot.value?.recentPackages ?? []).map((item) => item.path)),
  ),
);
const packageStats = usePackageStats(() => historyPaths.value);
const renderTelemetry = useRenderTelemetry();
function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}
function age(timestamp: number) {
  const minutes = Math.max(1, Math.floor((Date.now() - timestamp) / 60_000));
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h` : `${Math.floor(hours / 24)}d`;
}
</script>

<template>
  <section class="overview-page">
    <header class="overview-header">
      <div>
        <p class="eyebrow">{{ $t("home.eyebrow") }}</p>
        <FTypography :header="1" spacing="none">{{
          $t("home.title")
        }}</FTypography
        ><FTypography paragraphy type="secondary">{{
          $t("home.description")
        }}</FTypography>
      </div>
      <div class="overview-actions">
        <RouterLink class="secondary-action" to="/workspace"
          ><FIcon name="BookOpen" :size="16" aria-label="" />{{
            $t("home.openDocs")
          }}</RouterLink
        ><RouterLink class="primary-action" to="/packages"
          ><FIcon name="Package" :size="16" aria-label="" />{{
            $t("home.openPackage")
          }}</RouterLink
        >
      </div>
    </header>
    <p v-if="demo" class="demo-note" role="status">
      <FIcon name="Info" :size="16" aria-label="" />{{ $t("home.mockNotice") }}
    </p>
    <p v-if="error" class="error-message" role="alert">
      {{ error }}
    </p>
    <template v-if="snapshot"
      ><section class="metrics" :aria-label="$t('home.metrics')">
        <article class="metric-card">
          <FIcon name="Package" :size="18" aria-label="" /><span>{{
            $t("home.packages")
          }}</span
          ><strong>{{ snapshot.packages }}</strong>
        </article>
        <article class="metric-card">
          <FIcon name="Database" :size="18" aria-label="" /><span>{{
            $t("home.resources")
          }}</span
          ><strong>{{ snapshot.resources.toLocaleString() }}</strong>
        </article>
        <article class="metric-card">
          <FIcon name="Boxes" :size="18" aria-label="" /><span>{{
            $t("home.assets")
          }}</span
          ><strong>{{ snapshot.assets }}</strong>
        </article>
        <article class="metric-card metric-alert">
          <FIcon name="CircleAlert" :size="18" aria-label="" /><span>{{
            $t("home.failedOperations")
          }}</span
          ><strong>{{ snapshot.failedOperations }}</strong>
        </article>
      </section>
      <RuntimeLogsCard :events="snapshot.recentEvents" />
      <ExtensionStatsCards
        :stats="packageStats.stats.value"
        :loading="packageStats.loading.value"
        :error="packageStats.error.value"
      />
      <RenderStatsCard
        :summary="renderTelemetry.summary.value"
        :loading="renderTelemetry.loading.value"
        :error="renderTelemetry.error.value"
        @clear="renderTelemetry.clear"
      />
      <section class="content-grid">
        <article class="panel">
          <header class="panel-header">
            <div>
              <FTypography :header="3" spacing="none">{{
                $t("home.recentPackages")
              }}</FTypography
              ><FTypography paragraphy type="secondary">{{
                $t("home.recentPackagesDescription")
              }}</FTypography>
            </div>
            <RouterLink to="/packages">{{ $t("home.viewAll") }}</RouterLink>
          </header>
          <ul v-if="snapshot.recentPackages.length" class="package-list">
            <li v-for="item in snapshot.recentPackages" :key="item.id">
              <span class="list-icon"
                ><FIcon name="Package" :size="16" aria-label=""
              /></span>
              <div>
                <strong>{{ fileName(item.path) }}</strong>
                <p>
                  {{ item.entryCount.toLocaleString() }}
                  {{ $t("home.resources") }} · {{ item.openCount }}
                  {{ $t("home.opens") }}
                </p>
              </div>
              <time>{{ age(item.lastOpenedAt) }}</time>
            </li>
          </ul>
          <div v-else class="empty-state">
            <FIcon name="Package" :size="24" aria-label="" /><FTypography
              paragraphy
              type="secondary"
              >{{ $t("home.noPackages") }}</FTypography
            >
          </div>
        </article>
        <article class="panel">
          <header class="panel-header">
            <div>
              <FTypography :header="3" spacing="none">{{
                $t("home.activity")
              }}</FTypography
              ><FTypography paragraphy type="secondary">{{
                $t("home.activityDescription")
              }}</FTypography>
            </div>
            <FIcon
              name="Activity"
              :size="18"
              color="var(--primary)"
              aria-label=""
            />
          </header>
          <ul v-if="snapshot.recentEvents.length" class="event-list">
            <li v-for="event in snapshot.recentEvents" :key="event.id">
              <span class="event-dot" :class="event.level" aria-hidden="true" />
              <div>
                <strong>{{ event.message }}</strong>
                <p>{{ event.topic }} · {{ age(event.createdAt) }}</p>
              </div>
            </li>
          </ul>
          <div v-else class="empty-state">
            <FIcon name="Activity" :size="24" aria-label="" /><FTypography
              paragraphy
              type="secondary"
              >{{ $t("home.noActivity") }}</FTypography
            >
          </div>
        </article>
      </section></template
    >
    <div
      v-else-if="loading"
      class="overview-skeleton"
      role="status"
      :aria-label="$t('common.loading')"
    >
      <span class="sr-only">{{ $t("common.loading") }}</span>
      <div class="metrics" aria-hidden="true">
        <div v-for="n in 4" :key="n" class="metric-card">
          <FSkeleton width="65%" /><FSkeleton width="45%" height="32px" />
        </div>
      </div>
      <div class="content-grid" aria-hidden="true">
        <FSkeleton v-for="n in 2" :key="n" height="340px" />
      </div>
    </div>
  </section>
</template>

<style scoped>
.overview-skeleton {
  display: grid;
  gap: 24px;
}
.overview-page {
  display: grid;
  gap: 24px;
  max-width: 1440px;
  margin-inline: auto;
  min-width: 0;
  background: radial-gradient(
    ellipse at 65% 0%,
    color-mix(in srgb, var(--primary) 4%, transparent),
    transparent 55%
  );
}
.overview-header {
  align-items: center;
  display: flex;
  gap: 24px;
  justify-content: space-between;
  padding-bottom: 24px;
  border-bottom: 1px solid var(--border);
}
.overview-header :deep(h1) {
  font-size: clamp(26px, 2.4vw, 32px);
  font-weight: 600;
  letter-spacing: -0.04em;
  line-height: 1.2;
}
.overview-header :deep(p:not(.eyebrow)) {
  font-size: 13px;
  line-height: 1.7;
  margin: 10px 0 0;
  max-width: 62ch;
}
.overview-page :deep(h3) {
  font-size: 15px;
  font-weight: 600;
  letter-spacing: -0.015em;
  line-height: 1.5;
}
.overview-page :deep(.f-typography-secondary) {
  font-size: 12px;
  line-height: 1.65;
}
.eyebrow {
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  margin: 0 0 10px;
  text-transform: uppercase;
}
.overview-actions {
  display: flex;
  gap: 8px;
}
.primary-action,
.secondary-action {
  align-items: center;
  border-radius: var(--radius-md);
  display: inline-flex;
  font-size: 12px;
  font-weight: 700;
  gap: 7px;
  min-height: 38px;
  padding: 0 13px;
  text-decoration: none;
  transition:
    background-color 140ms ease,
    color 140ms ease,
    scale 140ms ease;
}
.primary-action {
  background: var(--primary);
  color: var(--primary-foreground);
}
.secondary-action {
  background: var(--surface);
  border: 1px solid var(--border);
  color: var(--foreground);
}
.primary-action:active,
.secondary-action:active {
  scale: 0.96;
}
.demo-note,
.panel,
.metric-card,
.loading-state {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
}
.demo-note {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 12px;
  gap: 8px;
  padding: 12px 14px;
}
.metrics {
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
}
.metric-card {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 18px 10px;
  padding: 20px 22px;
}
.metric-card > svg {
  color: var(--muted-foreground);
  grid-column: 2;
  grid-row: 1;
}
.metric-card span {
  grid-column: 1;
  grid-row: 1;
  color: var(--muted-foreground);
  font-size: 12px;
}
.metric-card strong {
  grid-column: 1 / -1;
  font-size: 32px;
  line-height: 1;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.04em;
}
.metric-alert > svg,
.metric-alert strong {
  color: var(--warning);
}
.content-grid {
  display: grid;
  gap: 14px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.panel {
  min-width: 0;
  padding: 20px;
}
.panel-header {
  align-items: flex-start;
  display: flex;
  gap: 12px;
  justify-content: space-between;
}
.panel-header > a {
  color: var(--primary);
  font-size: 12px;
  font-weight: 700;
  text-decoration: none;
}
.package-list,
.event-list {
  list-style: none;
  margin: 18px 0 0;
  max-height: clamp(240px, 38vh, 420px);
  overflow-y: auto;
  padding: 0;
}
.package-list li,
.event-list li {
  align-items: center;
  border-top: 1px solid var(--border);
  display: flex;
  gap: 11px;
  padding: 13px 0;
}
.list-icon {
  align-items: center;
  background: var(--accent);
  border-radius: var(--radius-sm);
  color: var(--primary);
  display: flex;
  height: 32px;
  justify-content: center;
  width: 32px;
}
.package-list div,
.event-list div {
  min-width: 0;
}
.package-list strong,
.event-list strong {
  font-size: 13px;
}
.package-list p,
.event-list p {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 4px 0 0;
}
.package-list time {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin-inline-start: auto;
}
.event-dot {
  background: var(--primary);
  border-radius: 50%;
  height: 7px;
  width: 7px;
}
.event-dot.warn {
  background: var(--warning);
}
.event-dot.error {
  background: var(--danger);
}
.empty-state,
.loading-state {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  gap: 8px;
  justify-content: center;
  min-height: 120px;
  text-align: center;
}
.loading-state {
  padding: 20px;
}
.error-message {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
}
@media (max-width: 850px) {
  .metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .content-grid {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 620px) {
  .overview-header {
    align-items: stretch;
    flex-direction: column;
  }
  .overview-actions {
    width: 100%;
  }
  .overview-actions a {
    flex: 1;
    justify-content: center;
  }
}
</style>
