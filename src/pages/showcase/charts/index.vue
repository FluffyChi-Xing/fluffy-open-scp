<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FChart from "@/components/extensions/FChart.vue";
const { t, locale } = useI18n({
  useScope: "local",
  missingWarn: false,
  fallbackWarn: false,
  messages: {
    "zh-CN": {
      requests: "请求",
      deployments: "部署",
      source: "来源：演示数据 · 每个点代表一天，空心点为周末",
    },
    "en-US": {
      requests: "Requests",
      deployments: "Deployments",
      source: "Source: demo data · One dot per day; hollow dots mark weekends",
    },
  },
});
const labels = computed(() =>
  Array.from({ length: 7 }, (_, i) =>
    new Intl.DateTimeFormat(locale.value, {
      weekday: "short",
      timeZone: "UTC",
    }).format(new Date(Date.UTC(2026, 9, 5 + i))),
  ),
);
const series = computed(() => [
  { name: t("requests"), values: [84, 119, 92, 158, 141, 187, 206] },
  { name: t("deployments"), values: [12, 18, 15, 25, 21, 30, 35] },
]);
</script>

<template>
  <section class="page">
    <header>
      <p class="eyebrow">{{ $t("showcase.eyebrow") }}</p>
      <h1>{{ $t("showcase.chartsTitle") }}</h1>
      <p>{{ $t("showcase.chartsDescription") }}</p>
    </header>
    <article class="panel">
      <div class="panel-heading">
        <div>
          <h2>{{ $t("showcase.traffic") }}</h2>
          <p>{{ $t("showcase.trafficDescription") }}</p>
        </div>
        <span>{{ $t("showcase.lastSevenDays") }}</span>
      </div>
      <FChart
        :labels="labels"
        :series="series"
        :label="$t('showcase.traffic')"
        class="chart"
      />
      <p class="source">{{ t("source") }}</p>
    </article>
  </section>
</template>

<style scoped>
.page {
  display: grid;
  gap: 24px;
}
.eyebrow {
  color: var(--primary);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.08em;
  margin: 0 0 10px;
  text-transform: uppercase;
}
.page h1 {
  font-size: clamp(1.8rem, 3vw, 2.5rem);
  letter-spacing: -0.045em;
  margin: 0;
}
.page header p:not(.eyebrow),
.panel-heading p {
  color: var(--muted-foreground);
  font-size: 14px;
  margin: 10px 0 0;
}
.panel {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  padding: 20px;
}
.panel-heading {
  align-items: flex-start;
  display: flex;
  justify-content: space-between;
}
.panel-heading h2 {
  font-size: 14px;
  margin: 0;
}
.panel-heading span {
  color: var(--subtle-foreground);
  font-size: 12px;
}
.source {
  color: var(--muted-foreground);
  font-size: 12px;
  border-top: 1px solid var(--border);
  padding-top: 14px;
}
.chart {
  margin-top: 16px;
  width: 100%;
}
</style>
