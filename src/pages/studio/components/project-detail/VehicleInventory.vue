<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{ sample?: Record<string, unknown>; running: boolean }>();
const { t, n } = useI18n();
const vehicles = computed(
  () =>
    props.sample?.vehicles as
      | {
          counts: Partial<Record<"carW1" | "carW2" | "carW3", number>>;
          added: number;
          removed: number;
        }
      | undefined,
);
const counts = computed(() => vehicles.value?.counts ?? {});
const total = computed(() => {
  const amounts = Object.values(counts.value);
  return amounts.length
    ? amounts.reduce((sum, amount) => sum + amount, 0)
    : undefined;
});
const state = computed(() => {
  if (!props.sample?.ready) return "vehicleWaiting";
  if (!props.sample.matched) return "vehicleNoInstance";
  if (total.value === undefined) return "vehicleNoInventory";
  return total.value > 0 ? "vehicleOccupied" : "vehicleEmpty";
});
</script>

<template>
  <section
    class="vehicle-inventory"
    :aria-label="t('assetCapture.filter.vehicles')"
  >
    <div class="vehicle-heading">
      <strong>{{ t("assetCapture.filter.vehicles") }}</strong>
      <span>{{ t(`assetCapture.${state}`) }}</span>
    </div>
    <dl class="vehicle-readings">
      <div>
        <dt>{{ t(running ? "assetCapture.vehiclesNow" : "assetCapture.vehiclesLast") }}</dt>
        <dd>{{ total === undefined ? "—" : n(total) }}</dd>
      </div>
      <div>
        <dt>{{ t("assetCapture.vehiclesAdded") }}</dt>
        <dd>{{ total === undefined ? "—" : n(vehicles?.added ?? 0) }}</dd>
      </div>
      <div>
        <dt>{{ t("assetCapture.vehiclesRemoved") }}</dt>
        <dd>{{ total === undefined ? "—" : n(vehicles?.removed ?? 0) }}</dd>
      </div>
      <div v-for="tier in ['carW1', 'carW2', 'carW3'] as const" :key="tier">
        <dt>{{ t(`assetCapture.resource.${tier}`) }}</dt>
        <dd>{{ counts[tier] === undefined ? "—" : n(counts[tier]) }}</dd>
      </div>
    </dl>
    <p>{{ t("assetCapture.vehicleSampling") }}</p>
  </section>
</template>

<style scoped>
.vehicle-inventory {
  display: grid;
  gap: 10px;
  padding: 12px 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
}
.vehicle-heading {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 8px;
}
.vehicle-heading strong {
  font-size: 13px;
}
.vehicle-heading span,
.vehicle-readings dt,
p {
  color: var(--muted-foreground);
  font-size: 12px;
}
.vehicle-readings {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
  gap: 12px;
  margin: 0;
}
.vehicle-readings dd {
  margin: 2px 0 0;
  font-size: 20px;
  font-variant-numeric: tabular-nums;
}
p {
  margin: 0;
}
</style>
