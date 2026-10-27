<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";
import FSelect, { type SelectOption } from "@/components/ui/FSelect.vue";

const props = defineProps<{ menuId: string }>();
const { t } = useI18n();
const store = useUiWorkbenchStore();
const name = shallowRef("");
const target = shallowRef("");
const selected = ref<string[]>([]);
const parentId = computed(() => store.menuGroups.find(group => group.id === props.menuId)?.parentId ?? props.menuId);
const available = computed(() => parentId.value !== "3193C0BE" && store.rootMenus.some(menu => menu.id === parentId.value));
const groups = computed(() => store.menuGroups.filter(group => group.parentId === parentId.value));
const entries = computed(() => store.menuEntries(props.menuId));
const targets = computed(() => store.moveTargets.filter(target => target.id !== props.menuId));
const browseOptions = computed<SelectOption[]>(() => [
  { value: parentId.value, label: t('studio.workbench.parentMenu') },
  ...groups.value.map(group => ({ value: group.id, label: group.label })),
]);
const moveOptions = computed<SelectOption[]>(() => [
  { value: "", label: t('studio.workbench.copyTargetPlaceholder') },
  ...targets.value.map(item => ({ value: item.id, label: item.label })),
]);
watch(() => props.menuId, () => { selected.value = []; target.value = ""; name.value = ""; });

function create(): void {
  const id = store.createMenuGroup(parentId.value, name.value);
  if (!id) return;
  for (const item of selected.value) store.moveEntry(props.menuId, item, id);
  selected.value = [];
  name.value = "";
  // Keep the parent menu visible after creation. Its existing entries become
  // the implicit “ungrouped” tab, so users can move them into the new group.
  store.enterMenu(parentId.value);
}
function move(): void {
  for (const item of selected.value) store.moveEntry(props.menuId, item, target.value);
  selected.value = [];
}
</script>

<template>
  <details v-if="available" class="grouping">
    <summary class="grouping-title">{{ t('studio.workbench.grouping') }}</summary>
    <div class="grouping-body">
      <p class="hint">{{ t('studio.workbench.groupingHint') }}</p>
      <label class="field">
        <span>{{ t('studio.workbench.browseGroup') }}</span>
        <FSelect :model-value="menuId" :options="browseOptions" @update:model-value="store.enterMenu" />
      </label>
      <fieldset class="members">
        <legend>{{ t('studio.workbench.selectGroupItems') }}</legend>
        <label v-for="entry in entries" :key="entry.tool.id" class="member">
          <input v-model="selected" type="checkbox" :value="entry.tool.id" />
          <span>{{ entry.tool.label }}</span>
        </label>
        <p v-if="entries.length === 0" class="hint">{{ t('studio.workbench.emptyMenu') }}</p>
      </fieldset>
      <label class="field">
        <span>{{ t('studio.workbench.groupName') }}</span>
        <input v-model="name" maxlength="80" @keydown.enter.prevent="create" />
      </label>
      <button type="button" class="action" :disabled="!name.trim()" @click="create">
        {{ selected.length ? t('studio.workbench.groupSelected') : t('studio.workbench.createGroup') }}
      </button>
      <label class="field">
        <span>{{ t('studio.workbench.moveTo') }}</span>
        <FSelect v-model="target" :options="moveOptions" />
      </label>
      <button type="button" class="action" :disabled="!target || !selected.length" @click="move">
        {{ t('studio.workbench.moveSelected', { count: selected.length }) }}
      </button>
    </div>
  </details>
</template>

<style scoped>
.grouping { margin: 10px 12px; border: 1px solid var(--border); border-radius: var(--radius-sm); }
.grouping-title { padding: 8px 10px; cursor: pointer; font-size: 13px; font-weight: 600; }
.grouping-body { display: grid; gap: 8px; padding: 0 10px 10px; }
.field { display: grid; gap: 4px; font-size: 11px; }
.field input, .field :deep(.f-select), .action { width: 100%; min-height: 32px; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface); color: var(--foreground); font: inherit; font-size: 12px; padding: 0 8px; }
.field :deep(.f-select) { height: 32px; min-height: 32px; }
.members { max-height: 180px; overflow: auto; border: 1px solid var(--border); }
.member { display: flex; align-items: center; gap: 6px; padding: 4px 5px; font-size: 11px; }
.hint { font-size: 11px; line-height: 1.45; color: var(--muted-foreground); margin: 0; }
.action { cursor: pointer; }
.action:disabled { opacity: .45; cursor: default; }
</style>
