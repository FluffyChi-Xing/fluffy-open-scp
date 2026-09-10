<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSheet from "@/components/ui/FSheet.vue";
import FMarkdown from "@/components/markdown/FMarkdown.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import { tauriApi } from "@/api";
import type { ResourceAnnotation, Tgi } from "@/api/tauri";
import { useToast } from "@/composables/useToast";

/**
 * 资源批注面板：源文件右键"创建批注"打开；列出当前 TGI 的全部批注
 * （md 默认预览态，可切编辑），支持新建（topic 标签可复用已有或新建）。
 */

const props = defineProps<{
  open: boolean;
  packagePath: string;
  tgi: Tgi | null;
  resourceLabel: string;
}>();
const emit = defineEmits<{ close: [void] }>();
const { t } = useI18n();
const toast = useToast();

const annotations = ref<ResourceAnnotation[]>([]);
const loading = ref(false);
const editingId = ref<number | null>(null);
const formTopic = ref("");
const formTitle = ref("");
const formContent = ref("");
const previewMode = ref(true);
const saving = ref(false);
const knownTopics = ref<string[]>([]);

async function load() {
  if (!props.tgi) return;
  loading.value = true;
  try {
    annotations.value = await tauriApi.annotations.forTgi(props.tgi);
    const stats = await tauriApi.annotations.topicStats();
    knownTopics.value = stats.map((entry) => entry.topic);
  } catch {
    annotations.value = [];
  } finally {
    loading.value = false;
  }
}

watch(
  () => [props.open, props.tgi],
  ([open]) => {
    if (open) {
      editingId.value = null;
      previewMode.value = true;
      void load();
    }
  },
);

function startCreate() {
  editingId.value = 0;
  formTopic.value = annotations.value[0]?.topic ?? "";
  formTitle.value = "";
  formContent.value = "";
  previewMode.value = false;
}

function startEdit(item: ResourceAnnotation) {
  editingId.value = item.id;
  formTopic.value = item.topic;
  formTitle.value = item.title;
  formContent.value = item.content;
  previewMode.value = false;
}

async function save() {
  if (!props.tgi || saving.value) return;
  if (!formTopic.value.trim() || !formTitle.value.trim()) {
    toast.error(t("notes.topicTitleRequired"));
    return;
  }
  saving.value = true;
  try {
    if (editingId.value && editingId.value > 0) {
      await tauriApi.annotations.update(
        editingId.value,
        formTopic.value,
        formTitle.value,
        formContent.value,
      );
    } else {
      await tauriApi.annotations.create({
        packagePath: props.packagePath,
        typeId: props.tgi.typeId,
        groupId: props.tgi.group,
        instance: props.tgi.instance,
        topic: formTopic.value,
        title: formTitle.value,
        content: formContent.value,
      });
    }
    editingId.value = null;
    previewMode.value = true;
    await load();
  } catch (error) {
    toast.error(error instanceof Error ? error.message : t("notes.saveFailed"));
  } finally {
    saving.value = false;
  }
}

async function remove(id: number) {
  try {
    await tauriApi.annotations.remove(id);
    await load();
  } catch {
    toast.error(t("notes.deleteFailed"));
  }
}

const editing = computed(() =>
  editingId.value === null || editingId.value === 0 ? null : editingId.value,
);
</script>

<template>
  <FSheet
    :open="props.open"
    :label="$t('notes.sheetTitle')"
    @update:open="emit('close')"
  >
    <div class="notes-sheet">
      <header class="notes-header">
        <div class="notes-header-text">
          <strong>{{ props.resourceLabel }}</strong>
          <code v-if="props.tgi" class="notes-tgi">
            0x{{ props.tgi.typeId.toString(16).padStart(8, "0") }}:0x{{
              props.tgi.group.toString(16).padStart(8, "0")
            }}:0x{{ props.tgi.instance.toString(16).padStart(8, "0") }}
          </code>
        </div>
        <button
          v-if="editingId === null"
          class="notes-create"
          type="button"
          @click="startCreate"
        >
          <FIcon name="Plus" :size="13" aria-label="" />
          {{ $t("notes.create") }}
        </button>
      </header>

      <p v-if="loading" class="notes-hint">{{ $t("common.loading") }}</p>
      <FEmpty
        v-else-if="!annotations.length && editingId === null"
        icon-name="StickyNote"
        :title="$t('notes.emptyTitle')"
        :desc="$t('notes.empty')"
      />

      <div
        v-if="editingId !== null"
        class="notes-form"
        :aria-label="$t('notes.formLabel')"
      >
        <label class="notes-field">
          <span>{{ $t("notes.topic") }}</span>
          <input
            v-model="formTopic"
            list="notes-topics"
            :placeholder="$t('notes.topicPlaceholder')"
          />
          <datalist id="notes-topics">
            <option v-for="topic in knownTopics" :key="topic" :value="topic" />
          </datalist>
        </label>
        <label class="notes-field">
          <span>{{ $t("notes.title") }}</span>
          <input v-model="formTitle" :placeholder="$t('notes.titlePlaceholder')" />
        </label>
        <div class="notes-field">
          <div class="notes-content-head">
            <span>{{ $t("notes.content") }}</span>
            <button
              class="notes-toggle"
              type="button"
              @click="previewMode = !previewMode"
            >
              <FIcon :name="previewMode ? 'SquarePen' : 'Eye'" :size="13" aria-label="" />
              {{ previewMode ? $t("notes.edit") : $t("notes.preview") }}
            </button>
          </div>
          <FMarkdown
            v-if="previewMode && formContent"
            :source="formContent"
            class="notes-md"
          />
          <textarea
            v-else
            v-model="formContent"
            rows="10"
            :placeholder="$t('notes.contentPlaceholder')"
          />
        </div>
        <div class="notes-actions">
          <button class="notes-primary" type="button" :disabled="saving" @click="save">
            {{ saving ? $t("package.exporting") : $t("notes.save") }}
          </button>
          <button
            type="button"
            @click="editingId = null; previewMode = true"
          >
            {{ $t("notes.cancel") }}
          </button>
        </div>
      </div>

      <article
        v-for="item in annotations"
        v-show="editingId === null || editing === item.id"
        :key="item.id"
        class="notes-item"
      >
        <div class="notes-item-head">
          <span class="notes-topic-chip">{{ item.topic }}</span>
          <strong>{{ item.title }}</strong>
          <span class="notes-meta">{{ new Date(item.updatedAt).toLocaleString() }}</span>
          <span class="notes-actions-inline">
            <button type="button" :aria-label="$t('notes.edit')" @click="startEdit(item)">
              <FIcon name="SquarePen" :size="13" aria-label="" />
            </button>
            <button type="button" :aria-label="$t('common.delete')" @click="remove(item.id)">
              <FIcon name="Trash2" :size="13" aria-label="" />
            </button>
          </span>
        </div>
        <FMarkdown :source="item.content || '*—*'" class="notes-md" />
      </article>
    </div>
  </FSheet>
</template>

<style scoped>
.notes-sheet {
  display: grid;
  gap: 14px;
  padding: 16px 16px 24px;
}
.notes-header {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 10px;
  justify-content: space-between;
  padding-bottom: 10px;
}
.notes-header-text {
  display: grid;
  gap: 4px;
  min-width: 0;
}
.notes-tgi {
  color: var(--muted-foreground);
  font: 11px ui-monospace, Consolas, monospace;
}
.notes-create,
.notes-primary,
.notes-actions button,
.notes-actions-inline button {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  gap: 6px;
  min-height: 28px;
  padding: 0 10px;
}
.notes-create:hover,
.notes-primary:hover,
.notes-actions button:hover,
.notes-actions-inline button:hover {
  background: var(--accent);
}
.notes-primary {
  background: var(--primary);
  border-color: var(--primary);
  color: var(--primary-foreground, white);
  font-weight: 650;
}
.notes-primary:disabled {
  cursor: wait;
  opacity: 0.6;
}
.notes-hint {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
}
.notes-form {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: grid;
  gap: 12px;
  padding: 12px;
}
.notes-field {
  display: grid;
  gap: 5px;
  font-size: 12px;
}
.notes-field > span {
  color: var(--muted-foreground);
}
.notes-field input,
.notes-field textarea {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  padding: 7px 9px;
}
.notes-field textarea {
  font-family: ui-monospace, Consolas, monospace;
  resize: vertical;
}
.notes-content-head {
  align-items: center;
  display: flex;
  justify-content: space-between;
}
.notes-toggle {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 5px;
  min-height: 24px;
  padding: 0 8px;
}
.notes-toggle:hover {
  background: var(--accent);
}
.notes-actions {
  display: flex;
  gap: 8px;
}
.notes-item {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: grid;
  gap: 8px;
  padding: 12px;
}
.notes-item-head {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.notes-topic-chip {
  background: var(--accent);
  border-radius: 999px;
  color: var(--foreground);
  font-size: 11px;
  padding: 2px 9px;
}
.notes-meta {
  color: var(--muted-foreground);
  font-size: 11px;
  margin-left: auto;
}
.notes-actions-inline {
  display: inline-flex;
  gap: 4px;
}
.notes-actions-inline button {
  min-height: 24px;
  padding: 0 6px;
}
.notes-md {
  font-size: 12px;
}
.notes-md :deep(pre) {
  overflow-x: auto;
}
</style>
