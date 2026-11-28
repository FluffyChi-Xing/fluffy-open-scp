<script setup lang="ts">
import { computed, onMounted, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FInput from "@/components/ui/FInput.vue";
import { loadReplicaCatalogue } from "@/lib/game-ui/replica/loader";
import type { ReplicaData } from "@/lib/game-ui/replica/types";
import VanillaVariantMenuCard from "./VanillaVariantMenuCard.vue";
import { menuTargetsFor, type MenuDimension } from "./menuBinding";
import { vanillaMenuBindings } from "./vanillaMenuBindings";

defineProps<{ disabled: boolean; selectedId?: string }>();
const emit = defineEmits<{ lookup: [id: string, label: string] }>();
const { t } = useI18n();
const data = shallowRef<ReplicaData>();
const failed = shallowRef(false);
const dimension = shallowRef<MenuDimension>("city");
const fireCategory = "9FAC79C9";
const category = shallowRef(fireCategory);
const filter = shallowRef("");
const subtab = shallowRef<number>();
const targets = computed(() => menuTargetsFor(dimension.value));
const activeTarget = computed(() => targets.value.find((target) => target.id === category.value));
const subtabs = computed(() => data.value?.subtabs?.[category.value] ?? []);
const entries = computed(() => {
  const source = data.value;
  if (!source) return [];
  const query = filter.value.trim().toLocaleLowerCase();
  const members = subtab.value === undefined ? undefined : new Set(subtabs.value[subtab.value]?.instances);
  const tools = (source.tools[category.value] ?? []).filter((tool) => !members || members.has(tool.instance));
  return tools.flatMap((tool) => {
    const eligible = Boolean(vanillaMenuBindings(tool.instance));
    const matches = !query || `${tool.label} ${tool.instance}`.toLocaleLowerCase().includes(query);
    const children = (source.submenus?.[tool.instance] ?? []).filter((item) =>
      vanillaMenuBindings(item.instance)
      && (matches || `${item.name} ${item.instance}`.toLocaleLowerCase().includes(query)),
    );
    return (eligible && matches) || children.length
      ? [{ tool, children, eligible }]
      : [];
  });
});

function changeDimension(value: MenuDimension) {
  dimension.value = value;
  changeCategory(value === "city" ? fireCategory : menuTargetsFor(value)[0].id);
}

function changeCategory(value: string) {
  category.value = value;
  subtab.value = undefined;
}

onMounted(async () => {
  try {
    data.value = await loadReplicaCatalogue();
  } catch {
    failed.value = true;
  }
});
</script>

<template>
  <div class="menu-browser">
    <nav class="catalogue-nav" :aria-label="t('flow.variant.menuLookup')">
      <div class="dimensions">
        <button
          v-for="mode in (['city', 'development'] as const)" :key="mode" type="button"
          :aria-pressed="dimension === mode" :disabled="disabled" @click="changeDimension(mode)">
          {{ t(mode === "city" ? "flow.variant.city" : "flow.variant.development") }}
        </button>
      </div>
      <div class="categories">
        <button
          v-for="target in targets" :key="target.id" type="button" :aria-pressed="category === target.id"
          :disabled="disabled" @click="changeCategory(target.id)">
          {{ t(target.labelKey) }}
          <FIcon v-if="category === target.id" name="ChevronRight" :size="14" aria-label="" />
        </button>
      </div>
    </nav>
    <section class="catalogue-content" :aria-label="activeTarget && t(activeTarget.labelKey)">
      <header class="catalogue-heading">
        <h3 v-if="activeTarget">{{ t(activeTarget.labelKey) }}</h3>
        <div class="filter">
          <FIcon name="Search" :size="15" aria-label="" />
          <FInput v-model="filter" :aria-label="t('flow.variant.filterMenu')" :placeholder="t('flow.variant.filterMenu')" />
        </div>
      </header>
      <div v-if="subtabs.length" class="subtabs">
        <button type="button" :aria-pressed="subtab === undefined" :disabled="disabled" @click="subtab = undefined">
          {{ t("flow.variant.allMenuEntries") }}
        </button>
        <button
          v-for="(tab, index) in subtabs" :key="tab.label" type="button" :aria-pressed="subtab === index"
          :disabled="disabled" @click="subtab = index">
          {{ tab.labelKey ? t(tab.labelKey) : tab.label }}
        </button>
      </div>
      <div v-if="failed || !data || !entries.length" class="catalogue-empty" role="status">
        <FIcon :name="failed ? 'CircleAlert' : 'Search'" :size="22" aria-label="" />
        {{ t(failed ? "flow.variant.menuLoadFailed" : !data ? "common.loading" : "flow.variant.noMenuEntries") }}
      </div>
      <ul v-else class="menu-entries">
        <li v-for="{tool, children, eligible} in entries" :key="tool.instance" class="menu-entry">
          <VanillaVariantMenuCard
            v-if="eligible" :label="tool.label || tool.instance" :preview="tool.preview"
            :selected="selectedId === tool.instance" :disabled="disabled"
            @click="emit('lookup', tool.instance, tool.label)" />
          <p v-else class="menu-parent">{{ tool.label || tool.instance }}</p>
          <details v-if="children.length" :open="!eligible || Boolean(filter.trim()) || children.some(item => item.instance === selectedId)">
            <summary>{{ t("flow.variant.modules") }}</summary>
            <ul>
              <li v-for="item in children" :key="item.instance">
                <VanillaVariantMenuCard
                  :label="item.name || item.instance" :preview="item.preview" :selected="selectedId === item.instance"
                  :disabled="disabled" @click="emit('lookup', item.instance, item.name)" />
              </li>
            </ul>
          </details>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.menu-browser { display: grid; grid-template-columns: minmax(145px, 26%) minmax(0, 1fr); min-block-size: 360px; }
.catalogue-nav { display: grid; align-content: start; gap: 10px; min-width: 0; padding: 10px; background: var(--sidebar); border-inline-end: 1px solid var(--border); border-end-start-radius: var(--radius-xl); }
.dimensions, .categories { display: grid; gap: 2px; }
.dimensions { grid-template-columns: repeat(2, minmax(0, 1fr)); background: var(--background); padding: 3px; border-radius: var(--radius-md); }
.dimensions button, .subtabs button, .categories button { min-height: 34px; padding: 5px 8px; color: var(--muted-foreground); font-size: 12px; border-radius: var(--radius-sm); }
.dimensions button { padding-inline: 4px; line-height: 1.25; }
.dimensions button[aria-pressed="true"] { background: var(--surface-elevated); color: var(--foreground); box-shadow: var(--shadow-sm); }
.categories { max-height: min(54vh, 440px); overflow: auto; scrollbar-gutter: stable; padding: 2px; }
.categories button { display: flex; justify-content: space-between; align-items: center; gap: 4px; text-align: start; min-height: 36px; }
.categories button[aria-pressed="true"], .subtabs button[aria-pressed="true"] { background: var(--accent); color: var(--accent-foreground); font-weight: 600; }
button:hover:not(:disabled):not([aria-pressed="true"]), summary:hover { background: var(--surface-hover); color: var(--foreground); }
button:focus-visible, summary:focus-visible { outline: 2px solid var(--ring); outline-offset: -2px; }
button:disabled { opacity: .6; }
.catalogue-content { min-width: 0; padding: 14px; }
.catalogue-heading { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 8px 12px; margin-block-end: 12px; }
h3 { font-weight: 650; font-size: 16px; }
.filter { position: relative; width: min(100%, 240px); }
.filter :deep(svg) { position: absolute; inset-inline-start: 10px; top: 50%; translate: 0 -50%; color: var(--muted-foreground); pointer-events: none; }
.filter :deep(input) { padding-inline-start: 32px; font-size: 13px; }
.subtabs { display: flex; flex-wrap: wrap; gap: 4px; margin-block-end: 10px; }
.catalogue-empty { display: grid; justify-items: center; align-content: center; gap: 10px; min-height: 180px; padding: 16px; text-align: center; color: var(--muted-foreground); font-size: 13px; }
.menu-entries { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 215px), 1fr)); align-items: start; gap: 8px; max-height: min(55vh, 460px); overflow: auto; scrollbar-gutter: stable; padding: 2px; }
.menu-entry { min-width: 0; }
.menu-parent { padding: 8px; font-weight: 600; font-size: 13px; }
details { padding-inline: 2px; color: var(--muted-foreground); }
summary { cursor: pointer; border-radius: var(--radius-sm); padding: 6px; font-size: 12px; }
details ul { display: grid; gap: 5px; padding-inline-start: 9px; margin-inline-start: 6px; border-inline-start: 1px solid var(--border); }
@container (max-width: 590px) {
  .menu-browser { grid-template-columns: minmax(0, 1fr); }
  .catalogue-nav { border-inline-end: 0; border-block-end: 1px solid var(--border); border-radius: 0; }
  .dimensions { width: min(100%, 240px); }
  .categories { display: flex; overflow-x: auto; scrollbar-gutter: auto; }
  .categories button { flex: none; }
}
</style>
