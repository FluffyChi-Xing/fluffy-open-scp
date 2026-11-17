<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import FSelect from "@/components/ui/FSelect.vue";
import { buildingGroups, developmentGroups } from "./cityCatalog";

defineProps<{ disabled: boolean }>();
const model = defineModel<string>({ required: true });
const { t } = useI18n();
const captureScopes = ["ui", "building", "agent", "map"] as const;
type CaptureScope = typeof captureScopes[number];
const scope = shallowRef<CaptureScope>("ui");
const section = shallowRef("city");
const category = shallowRef("power");

const scopes = computed(() =>
  captureScopes.map((value) => ({
    value,
    label: t(value === "building" ? "observerMenu.buildings"
      : value === "map" ? "observerMenu.mapResources" : `runtimeObserver.scope.${value}.label`),
  })),
);
const sections = computed(() =>
  ["city", "development"].map((value) => ({
    value,
    label: t(`observerMenu.${value}`),
  })),
);
const groups = computed<Record<string, Record<string, number>>>(() =>
  section.value === "city" ? buildingGroups : developmentGroups,
);
const categories = computed(() =>
  Object.keys(groups.value).map((value) => ({
    value,
    label: t(`observerMenu.${value}`),
  })),
);
const units = computed(() =>
  Object.entries(groups.value[category.value] ?? {}).map(([key, id]) => ({
    value: `city:${id}`,
    label: t(`observerMenu.units.${key}`),
  })),
);
const agents = computed(() =>
  ["police", "fire", "ambulance", "bus", "garbage"].map((value) => ({
    value: `agent:${value}`,
    label: t(`observerMenu.agents.${value}`),
  })),
);

function chooseScope(value: CaptureScope) {
  scope.value = value;
  model.value = {
    ui: "ui:all",
    map: "map:resources",
    agent: agents.value[0].value,
    building: units.value[0]?.value ?? "",
  }[value];
}
function chooseSection(value: string) {
  section.value = value;
  chooseCategory(value === "city" ? "power" : "specialization");
}
function chooseCategory(value: string) {
  category.value = value;
  model.value = units.value[0]?.value ?? "";
}
</script>

<template>
  <div class="filters">
    <div
      class="filter-row"
      role="group"
      :aria-label="t('runtimeObserver.captureScope')"
    >
      <span>{{ t("runtimeObserver.captureScope") }}</span>
      <button
        v-for="item in scopes"
        :key="item.value"
        type="button"
        :disabled="disabled"
        :aria-pressed="scope === item.value"
        @click="chooseScope(item.value)"
      >
        {{ item.label }}
      </button>
    </div>
    <template v-if="scope === 'building'">
      <div class="filter-row" role="group" :aria-label="t('observerMenu.menu')">
        <span>{{ t("observerMenu.menu") }}</span>
        <button
          v-for="item in sections"
          :key="item.value"
          type="button"
          :disabled="disabled"
          :aria-pressed="section === item.value"
          @click="chooseSection(item.value)"
        >
          {{ item.label }}
        </button>
      </div>
      <div
        class="filter-row"
        role="group"
        :aria-label="t('observerMenu.category')"
      >
        <span>{{ t("observerMenu.category") }}</span>
        <button
          v-for="item in categories"
          :key="item.value"
          type="button"
          :disabled="disabled"
          :aria-pressed="category === item.value"
          @click="chooseCategory(item.value)"
        >
          {{ item.label }}
        </button>
      </div>
      <div
        v-if="units.length"
        class="filter-row"
        role="group"
        :aria-label="t('runtimeObserver.captureTarget')"
      >
        <span>{{ t("runtimeObserver.captureTarget") }}</span>
        <button
          v-for="item in units"
          :key="item.value"
          type="button"
          :disabled="disabled"
          :aria-pressed="model === item.value"
          @click="model = item.value"
        >
          {{ item.label }}
        </button>
      </div>
      <p v-if="units.length">{{ t("observerMenu.snapshotHint") }}</p>
    </template>
    <div v-else-if="scope === 'agent'" class="filter-row">
      <span>{{ t("runtimeObserver.captureTarget") }}</span>
      <div class="agent-select">
        <FSelect
          v-model="model"
          :options="agents"
          :disabled="disabled"
          :aria-label="t('runtimeObserver.captureTarget')"
        />
      </div>
      <p>{{ t("runtimeObserver.scope.agent.hint") }}</p>
    </div>
    <p v-else-if="scope === 'map'">{{ t("observerMenu.mapHint") }}</p>
    <p v-else>{{ t("runtimeObserver.scope.ui.hint") }}</p>
  </div>
</template>

<style scoped>
.filters {
  display: grid;
  gap: 5px;
  min-inline-size: 0;
}
.filter-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px;
}
.filter-row > span {
  min-inline-size: 7em;
  color: var(--muted-foreground);
  font-size: 12px;
}
.filter-row > button {
  padding: 4px 8px;
  min-block-size: 26px;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
}
.filter-row > button[aria-pressed="true"] {
  border-color: var(--accent);
  background: var(--surface-elevated);
}
.filter-row > button:focus-visible {
  outline: 2px solid var(--ring);
}
.agent-select {
  inline-size: 180px;
}
p {
  margin: 0;
  font-size: 12px;
  color: var(--muted-foreground);
}
</style>
