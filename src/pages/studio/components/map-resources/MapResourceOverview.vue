<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeft, ArrowUpRight, Map } from "lucide-vue-next";
import FButton from "@/components/ui/FButton.vue";
import type { Region3DData } from "@/lib/region-map";

const props = defineProps<{
  data: Region3DData | null;
  group: string;
  abundance: ReadonlyMap<string, ReadonlyArray<{ key: string; color: string; coverage: number | null; changed: boolean }>>;
}>();
const emit = defineEmits<{ back: []; select: [uid: string] }>();
const { t, locale, n } = useI18n();
const chinese = computed(() => locale.value.startsWith("zh"));
const cities = computed(() => props.data?.plots.filter((plot) => plot.kind === "city") ?? []);
const title = computed(() => (chinese.value ? props.data?.displayName : props.data?.displayNameEn)
  ?? props.data?.displayName ?? props.data?.displayNameEn ?? props.group);
</script>

<template>
  <section class="overview" :aria-label="t('mapResources.project')">
    <FButton variant="ghost" class="back" @click="emit('back')">
      <ArrowLeft :size="15" aria-hidden="true" /> {{ t("mapResources.returnToMap") }}
    </FButton>
    <header v-if="data" class="region">
      <div class="region-mark"><Map :size="24" aria-hidden="true" /></div>
      <div class="region-name">
        <p>{{ t("mapResources.region") }}</p>
        <h2>{{ title }}</h2>
      </div>
      <div class="region-facts">
        <span>{{ t("mapResources.regionId") }} <code>{{ group }}</code></span>
        <span>{{ t("mapResources.cityCount", { count: cities.length }) }}</span>
      </div>
    </header>
    <p v-else class="empty">{{ t("mapResources.noRegion") }}</p>
    <div v-if="data" class="city-grid">
      <article v-for="city in cities" :key="city.uid" class="city-card">
        <div class="city-header">
          <div class="city-name">
            <p>{{ t("mapResources.citySlot", { id: city.uid }) }}</p>
            <h3>{{ (chinese ? city.name : city.nameEn) ?? city.name ?? city.nameEn ?? city.uid }}</h3>
          </div>
          <FButton
variant="secondary" class="open-city" :title="t('mapResources.openCity')"
            :aria-label="t('mapResources.openCityLabel', { name: (chinese ? city.name : city.nameEn) ?? city.name ?? city.nameEn ?? city.uid })" @click="emit('select', city.uid)">
            <ArrowUpRight :size="17" aria-hidden="true" />
          </FButton>
        </div>
        <div class="city-meta">
          <span>{{ t("mapResources.coordinates") }} <code>{{ n(city.x) }}, {{ n(city.y) }}</code></span>
          <span :class="{ changed: abundance.get(city.uid)?.some((layer) => layer.changed) }">
            {{ t(abundance.get(city.uid)?.some((layer) => layer.changed) ? "mapResources.modified" : "mapResources.unchanged") }}
          </span>
        </div>
        <h4>{{ t("mapResources.resources") }}</h4>
        <ul v-if="abundance.has(city.uid)" class="resources">
          <li v-for="layer in abundance.get(city.uid)" :key="layer.key" class="resource">
            <span class="label"><i :style="{ background: layer.color }" />{{ t(`mapResources.${layer.key}`) }}</span>
            <span class="track" aria-hidden="true"><i :style="{ width: `${layer.coverage ?? 0}%`, background: layer.color }" /></span>
            <span class="number">{{ layer.coverage === null ? t("mapResources.unavailable") : t("mapResources.coverage", { value: n(layer.coverage, { maximumFractionDigits: 1 }) }) }}</span>
            <span v-if="layer.changed" class="changed layer-change">{{ t("mapResources.modified") }}</span>
          </li>
        </ul>
        <p v-else class="empty">{{ t("mapResources.unavailable") }}</p>
      </article>
    </div>
  </section>
</template>

<style scoped>
.overview { height: 100%; overflow: auto; padding: clamp(16px, 3vw, 36px); background: var(--background); color: var(--foreground); }
.back { margin-bottom: 16px; }
.region { display: flex; flex-wrap: wrap; align-items: center; gap: 16px; padding: 22px; background: var(--surface); border: 1px solid var(--border); border-inline-start: 3px solid var(--primary); border-radius: var(--radius-md); }
.region-mark { display: grid; place-items: center; width: 48px; height: 48px; color: var(--primary); background: var(--surface-hover); border-radius: var(--radius-md); }
.region-name { flex: 1; min-width: 130px; }
.region-name p, .city-name p { margin: 0 0 3px; color: var(--muted-foreground); font-size: 12px; }
.region-name h2 { font-size: 26px; font-weight: 650; margin: 0; }
.region-facts { display: grid; gap: 6px; color: var(--muted-foreground); font-size: 12px; }
.region-facts span, .city-meta span { display: inline-flex; align-items: baseline; gap: 6px; }
.city-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 315px), 1fr)); gap: 14px; margin-top: 18px; }
.city-card { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-md); padding: 16px; min-width: 0; }
.city-header, .city-meta { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.city-name h3 { margin: 0; font-size: 17px; font-weight: 650; overflow-wrap: anywhere; }
.open-city { flex-shrink: 0; width: 38px; padding: 0; }
.city-meta { margin: 12px 0; flex-wrap: wrap; font-size: 11px; color: var(--muted-foreground); padding-bottom: 12px; border-bottom: 1px solid var(--border); }
.changed { color: var(--success); }
h4 { margin: 0 0 10px; color: var(--muted-foreground); font-size: 11px; font-weight: 600; }
.resources { display: grid; gap: 7px; margin: 0; padding: 0; list-style: none; }
.resource { display: grid; grid-template-columns: minmax(0, 9em) minmax(18px, 1fr) minmax(0, 4em); gap: 8px; align-items: center; font-size: 11px; }
.label { display: flex; align-items: center; gap: 6px; }
.label i { width: 6px; height: 6px; border-radius: 50%; }
.track { height: 4px; background: var(--border-strong); border-radius: 2px; overflow: hidden; }
.track i { display: block; height: 100%; }
.number { text-align: right; font-variant-numeric: tabular-nums; color: var(--muted-foreground); }
.layer-change { grid-column: 1 / -1; margin-top: -4px; font-size: 10px; }
.empty { color: var(--muted-foreground); }
</style>
