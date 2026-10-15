<script setup lang="ts">
import { computed } from "vue";

interface Props {
  labels: string[];
  series: { name: string; values: number[] }[];
  label: string;
}

const props = defineProps<Props>();
// Lieflat F2 / basics-gallery B2: calendar floor, hairline, daily dots, hollow weekends.
const maximum = computed(() =>
  Math.max(1, ...props.series.flatMap((s) => s.values)),
);
const x = (i: number) => 38 + (i * 524) / Math.max(1, props.labels.length - 1);
const y = (value: number) => 250 - (value / maximum.value) * 208;
const plots = computed(() =>
  props.series.map((series, index) => ({
    ...series,
    color: index === 0 ? "var(--foreground)" : "var(--muted-foreground)",
    path: series.values
      .map((value, i) => `${i ? "L" : "M"} ${x(i)} ${y(value)}`)
      .join(" "),
  })),
);
</script>

<template>
  <figure class="f-chart">
    <figcaption class="legend">
      <span v-for="(plot, index) in plots" :key="plot.name"
        ><i
          :style="{ background: plot.color }"
          :class="{ hollow: index > 0 }"
        />{{ plot.name }}</span
      >
    </figcaption>
    <svg viewBox="0 0 600 290" role="img" :aria-label="label">
      <g v-for="fraction in [0, 0.5, 1]" :key="fraction" class="axis">
        <line
          x1="38"
          x2="562"
          :y1="y(maximum * fraction)"
          :y2="y(maximum * fraction)"
        />
        <text x="28" :y="y(maximum * fraction) + 4" text-anchor="end">
          {{ Math.round(maximum * fraction) }}
        </text>
      </g>
      <g v-for="(day, i) in labels" :key="i" class="axis">
        <line :x1="x(i)" :x2="x(i)" y1="250" y2="243" />
        <text :x="x(i)" y="274" text-anchor="middle">{{ day }}</text>
      </g>
      <g
        v-for="(plot, index) in plots"
        :key="plot.name"
        :style="{ color: plot.color }"
      >
        <path
          :d="plot.path"
          class="hairline"
          :stroke-dasharray="index ? '4 4' : undefined"
        />
        <circle
          v-for="(value, i) in plot.values"
          :key="i"
          :cx="x(i)"
          :cy="y(value)"
          r="3.5"
          :fill="i % 7 >= 5 ? 'var(--surface)' : 'currentColor'"
          stroke="currentColor"
          tabindex="0"
        >
          <title>{{ labels[i] }} · {{ plot.name }}: {{ value }}</title>
        </circle>
      </g>
    </svg>
    <div class="values">
      <span v-for="(day, i) in labels" :key="i"
        ><strong>{{ day }}</strong
        ><span v-for="plot in plots" :key="plot.name"
          >{{ plot.name }} {{ plot.values[i] }}</span
        ></span
      >
    </div>
  </figure>
</template>

<style scoped>
.f-chart {
  min-width: 0;
  margin: 0;
}
.f-chart svg {
  display: block;
  width: 100%;
  max-height: 340px;
}
.axis line {
  stroke: var(--border);
  stroke-width: 1;
}
.axis text {
  fill: var(--muted-foreground);
  font-size: 11px;
}
.hairline {
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
}
.legend {
  display: flex;
  gap: 20px;
  font-size: 12px;
}
.legend span {
  display: flex;
  align-items: center;
  gap: 7px;
}
.legend i {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}
.values {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  color: var(--muted-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.values > span {
  display: grid;
  gap: 3px;
  flex: 1;
  min-width: 75px;
}
.values strong {
  color: var(--foreground);
  font-weight: 500;
}
</style>
