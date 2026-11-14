<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FSelect from "@/components/ui/FSelect.vue";
import { useModProjectsStore } from "@/stores/modProjects";
import { useToast } from "@/composables/useToast";
import type { ModProjectGroup, ModProjectView } from "@/api/tauri";

/**
 * Settings tab：内联编辑项目元数据 + 删除记录。
 * 状态使用 FSelect（禁止原生 select / radio group）。
 */
const props = defineProps<{
  project: ModProjectView;
  groups: ModProjectGroup[];
}>();

const emit = defineEmits<{
  remove: [project: ModProjectView];
}>();

const { t } = useI18n();
const store = useModProjectsStore();
const toast = useToast();
const name = ref("");
const groupId = ref<number | undefined>();
const description = ref("");
const status = ref("active");
const groupMenuOpen = ref(false);
const busy = ref(false);
const error = ref("");

const statusOptions = computed(() =>
  (["active", "released", "archived"] as const).map((value) => ({
    value,
    label: t(`studio.dashboard.status.${value}`),
  })),
);

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

watch(
  () =>
    [
      props.project.name,
      props.project.groupId,
      props.project.description,
      props.project.status,
    ] as const,
  () => syncFromProject(),
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
    await store.updateProject(props.project.id, {
      name: name.value.trim(),
      groupId: groupId.value,
      description: description.value.trim() || undefined,
      status: status.value,
    });
    toast.success(t("studio.repo.settingsSaved"));
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
    <header class="settings-head">
      <h2>{{ $t("studio.repo.settingsTitle") }}</h2>
      <p>{{ $t("studio.repo.settingsHint") }}</p>
    </header>

    <div class="settings-form">
      <label class="field">
        <span>{{ $t("studio.projects.form.nameLabel") }}</span>
        <input v-model="name" type="text" autocomplete="off" />
        <small>{{ $t("studio.projects.form.nameHint") }}</small>
      </label>

      <div class="field">
        <span>{{ $t("studio.projects.form.groupLabel") }}</span>
        <FDropdown v-model:open="groupMenuOpen" :width="260">
          <template #trigger>
            <button class="select-trigger" type="button">
              <span>{{ selectedGroupName }}</span>
              <FIcon name="ChevronDown" :size="13" aria-label="" />
            </button>
          </template>
          <button type="button" class="menu-item" @click="chooseGroup(undefined)">
            <FIcon
              :name="groupId === undefined ? 'Check' : 'Minus'"
              :size="14"
              aria-label=""
            />
            {{ $t("studio.projects.form.groupNone") }}
          </button>
          <button
            v-for="group in groups"
            :key="group.id"
            type="button"
            class="menu-item"
            @click="chooseGroup(group.id)"
          >
            <FIcon
              :name="groupId === group.id ? 'Check' : 'Tag'"
              :size="14"
              aria-label=""
            />
            {{ group.name }}
          </button>
        </FDropdown>
      </div>

      <label class="field">
        <span>{{ $t("studio.projects.form.statusLabel") }}</span>
        <FSelect v-model="status" :options="statusOptions" />
      </label>

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
        <button
          class="danger"
          type="button"
          :disabled="busy"
          @click="confirmDelete"
        >
          <FIcon name="Trash2" :size="13" aria-label="" />
          {{ $t("studio.projects.delete") }}
        </button>
      </div>
    </div>
  </article>
</template>

<style scoped>
.settings-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  display: grid;
  gap: 0;
  max-width: 560px;
  overflow: hidden;
}
.settings-head {
  border-bottom: 1px solid var(--border);
  display: grid;
  gap: 4px;
  padding: 16px 18px;
}
.settings-head h2 {
  font-size: 15px;
  font-weight: 650;
  line-height: 1.3;
  margin: 0;
}
.settings-head p {
  color: var(--muted-foreground);
  font-size: 12.5px;
  line-height: 1.45;
  margin: 0;
}
.settings-form {
  display: grid;
  gap: 16px;
  padding: 16px 18px 18px;
}
.field {
  display: grid;
  gap: 6px;
  min-width: 0;
}
.field :deep(.f-dropdown-anchor),
.field :deep(.f-select) {
  width: 100%;
}
.field > span {
  color: var(--muted-foreground);
  font-size: 11.5px;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.field input,
.field textarea,
.select-trigger {
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 13px;
  min-height: 36px;
  padding: 8px 10px;
  width: 100%;
}
.field textarea {
  line-height: 1.45;
  min-height: 96px;
  resize: vertical;
}
.field small {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.select-trigger {
  align-items: center;
  cursor: pointer;
  display: inline-flex;
  gap: 8px;
  justify-content: space-between;
  text-align: start;
}
.menu-item {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  gap: 8px;
  padding: 8px 10px;
  text-align: start;
  width: 100%;
}
.menu-item:hover {
  background: var(--surface-hover);
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
.actions button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.error {
  color: var(--destructive, #f87171);
  font-size: 12.5px;
  margin: 0;
}
</style>
