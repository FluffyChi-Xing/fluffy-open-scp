<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import type { ModProjectGroup, ModProjectView } from "@/api/tauri";

/**
 * Settings tab：内联编辑项目元数据 + 删除记录（原列表「编辑/删除」迁移至此）。
 */
const props = defineProps<{
  project: ModProjectView;
  groups: ModProjectGroup[];
}>();

const emit = defineEmits<{
  update: [
    id: number,
    fields: {
      name: string;
      groupId?: number;
      description?: string;
      status: string;
    },
  ];
  remove: [project: ModProjectView];
}>();

const { t } = useI18n();
const name = ref("");
const groupId = ref<number | undefined>();
const description = ref("");
const status = ref("active");
const groupMenuOpen = ref(false);
const busy = ref(false);
const error = ref("");

const statuses = ["active", "released", "archived"] as const;

const selectedGroupName = computed(
  () =>
    props.groups.find((group) => group.id === groupId.value)?.name ??
    t("studio.projects.form.groupNone"),
);

function syncFromProject() {
  name.value = props.project.name;
  groupId.value = props.project.groupId;
  description.value = props.project.description ?? "";
  status.value = props.project.status;
  error.value = "";
}

watch(
  () => props.project.id,
  () => syncFromProject(),
  { immediate: true },
);

function chooseGroup(id: number | undefined) {
  groupId.value = id;
  groupMenuOpen.value = false;
}

async function save() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await emit("update", props.project.id, {
      name: name.value.trim(),
      groupId: groupId.value,
      description: description.value.trim() || undefined,
      status: status.value,
    });
  } catch (cause) {
    error.value =
      cause && typeof cause === "object" && "message" in cause
        ? String((cause as { message: unknown }).message)
        : String(cause);
  } finally {
    busy.value = false;
  }
}

function confirmDelete() {
  if (!window.confirm(t("studio.projects.deleteConfirm"))) return;
  emit("remove", props.project);
}
</script>

<template>
  <article class="settings-card">
    <header>
      <h2>{{ $t("studio.repo.settingsTitle") }}</h2>
      <p>{{ $t("studio.repo.settingsHint") }}</p>
    </header>

    <label class="field">
      <span>{{ $t("studio.projects.form.nameLabel") }}</span>
      <input v-model="name" type="text" autocomplete="off" />
      <small>{{ $t("studio.projects.form.nameHint") }}</small>
    </label>

    <div class="field">
      <span>{{ $t("studio.projects.form.groupLabel") }}</span>
      <FDropdown v-model:open="groupMenuOpen">
        <template #trigger>
          <button class="group-trigger" type="button">
            {{ selectedGroupName }}
            <FIcon name="ChevronDown" :size="13" aria-label="" />
          </button>
        </template>
        <button type="button" class="menu-item" @click="chooseGroup(undefined)">
          {{ $t("studio.projects.form.groupNone") }}
        </button>
        <button
          v-for="group in groups"
          :key="group.id"
          type="button"
          class="menu-item"
          @click="chooseGroup(group.id)"
        >
          {{ group.name }}
        </button>
      </FDropdown>
    </div>

    <fieldset class="field status-field">
      <legend>{{ $t("studio.projects.form.statusLabel") }}</legend>
      <label v-for="item in statuses" :key="item" class="status-option">
        <input v-model="status" type="radio" :value="item" />
        {{ $t(`studio.dashboard.status.${item}`) }}
      </label>
    </fieldset>

    <label class="field">
      <span>{{ $t("studio.projects.form.descLabel") }}</span>
      <textarea
        v-model="description"
        rows="4"
        :placeholder="$t('studio.projects.form.descPlaceholder')"
      ></textarea>
    </label>

    <p v-if="error" class="error" role="alert">{{ error }}</p>

    <div class="actions">
      <button class="primary" type="button" :disabled="busy" @click="save">
        {{ $t("studio.projects.form.save") }}
      </button>
      <button class="danger" type="button" :disabled="busy" @click="confirmDelete">
        <FIcon name="Trash2" :size="13" aria-label="" />
        {{ $t("studio.projects.delete") }}
      </button>
    </div>
  </article>
</template>

<style scoped>
.settings-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  display: grid;
  gap: 14px;
  max-width: 560px;
  padding: 18px;
}
.settings-card header h2 {
  font-size: 15px;
  margin: 0 0 4px;
}
.settings-card header p {
  color: var(--muted-foreground);
  font-size: 12.5px;
  margin: 0;
}
.field {
  display: grid;
  gap: 6px;
}
.field > span,
.field legend {
  color: var(--muted-foreground);
  font-size: 11.5px;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.field input,
.field textarea,
.group-trigger {
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 13px;
  padding: 8px 10px;
}
.field small {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.group-trigger {
  align-items: center;
  cursor: pointer;
  display: inline-flex;
  gap: 8px;
  justify-content: space-between;
  width: 100%;
}
.menu-item {
  background: transparent;
  border: 0;
  color: var(--foreground);
  cursor: pointer;
  display: block;
  font: inherit;
  padding: 8px 10px;
  text-align: start;
  width: 100%;
}
.menu-item:hover {
  background: var(--surface-hover);
}
.status-field {
  border: 0;
  margin: 0;
  padding: 0;
}
.status-option {
  align-items: center;
  display: inline-flex;
  font-size: 13px;
  gap: 6px;
  margin-right: 14px;
}
.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 4px;
}
.actions button {
  align-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 6px;
  min-height: 34px;
  padding: 0 12px;
}
.actions .primary {
  background: var(--primary);
  border-color: var(--primary);
  color: var(--primary-foreground, #fff);
}
.actions .danger {
  background: transparent;
  color: var(--destructive, #f87171);
}
.error {
  color: var(--destructive, #f87171);
  font-size: 12.5px;
  margin: 0;
}
</style>
