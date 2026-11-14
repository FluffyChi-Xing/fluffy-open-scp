<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import { tauriApi } from "@/api";
import type { EffectAnnotation, Tgi, UnitKeyDto } from "@/api/tauri";
import { useToast } from "@/composables/useToast";

/**
 * 效果语义标签（「属性」页签 section，仅 effect 单元渲染）：选中 effect
 * 后按其 TGI 创建/复用语义 tag + 批注（全局键控，跨资产复用；source 记录
 * 当前 lot 资源，供批注 sheet 与笔记工作区消费）。空白效果（无 TGI）不可标。
 */
const props = defineProps<{
  effectId: UnitKeyDto | null;
  /** 创建时的来源 lot 资源 TGI；draft 工作台无 TGI 时缺省。 */
  sourceTgi?: Tgi | null;
}>();
const { t } = useI18n();
const toast = useToast();

const items = ref<EffectAnnotation[]>([]);
const knownTags = ref<string[]>([]);
const loading = ref(false);
const saving = ref(false);
const formTag = ref("");
const formNote = ref("");
const tagOpen = ref(false);

function hex(value: number) {
  return value.toString(16).padStart(8, "0").toUpperCase();
}

async function load() {
  const effectId = props.effectId;
  if (!effectId) {
    items.value = [];
    knownTags.value = [];
    return;
  }
  loading.value = true;
  try {
    const [list, stats] = await Promise.all([
      tauriApi.effectAnnotations.forEffect(effectId),
      tauriApi.effectAnnotations.tagStats(),
    ]);
    items.value = list;
    knownTags.value = stats.map((entry) => entry.tag);
  } catch {
    items.value = [];
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.effectId,
  () => {
    formTag.value = "";
    formNote.value = "";
    tagOpen.value = false;
    void load();
  },
  { immediate: true },
);

function pickTag(tag: string) {
  formTag.value = tag;
  tagOpen.value = false;
}

async function save() {
  const effectId = props.effectId;
  if (!effectId || saving.value) return;
  if (!formTag.value.trim()) {
    toast.error(t("effectTags.tagRequired"));
    return;
  }
  saving.value = true;
  try {
    await tauriApi.effectAnnotations.create({
      typeId: effectId.typeId,
      groupId: effectId.group,
      instance: effectId.instance,
      tag: formTag.value,
      note: formNote.value,
      sourceTypeId: props.sourceTgi?.typeId ?? null,
      sourceGroupId: props.sourceTgi?.group ?? null,
      sourceInstanceId: props.sourceTgi?.instance ?? null,
    });
    formTag.value = "";
    formNote.value = "";
    await load();
  } catch (error) {
    toast.error(error instanceof Error ? error.message : t("effectTags.saveFailed"));
  } finally {
    saving.value = false;
  }
}

async function remove(id: number) {
  try {
    await tauriApi.effectAnnotations.remove(id);
    await load();
  } catch {
    toast.error(t("effectTags.deleteFailed"));
  }
}
</script>

<template>
  <section class="effect-tags" :aria-label="t('effectTags.sectionTitle')">
    <h4 class="tags-title">{{ t("effectTags.sectionTitle") }}</h4>
    <p v-if="!props.effectId" class="tags-note">{{ t("effectTags.noEffectId") }}</p>
    <template v-else>
      <p v-if="!loading && !items.length" class="tags-note">
        {{ t("effectTags.sectionEmpty") }}
      </p>
      <article v-for="item in items" :key="item.id" class="tag-item">
        <header class="tag-item-head">
          <span class="tag-chip">{{ item.tag }}</span>
          <code
            class="tag-tgi"
            :title="`T ${hex(item.typeId)} · G ${hex(item.groupId)} · I ${hex(item.instance)}`"
          >
            {{ hex(item.instance) }}
          </code>
          <button
            type="button"
            class="tag-remove"
            :aria-label="t('common.delete')"
            @click="remove(item.id)"
          >
            <FIcon name="Trash2" :size="13" aria-label="" />
          </button>
        </header>
        <p v-if="item.note" class="tag-note-text">{{ item.note }}</p>
      </article>

      <div class="tag-form">
        <div class="tags-field">
          <span>{{ t("effectTags.tag") }}</span>
          <FDropdown v-model:open="tagOpen" :width="240">
            <template #trigger>
              <button
                type="button"
                class="tag-trigger"
                :class="{ placeholder: !formTag }"
              >
                <span class="tag-trigger-text">
                  {{ formTag || t("effectTags.tagPlaceholder") }}
                </span>
                <FIcon name="ChevronDown" :size="14" aria-label="" />
              </button>
            </template>
            <input
              v-model="formTag"
              class="tag-input"
              :placeholder="t('effectTags.tagInput')"
              @keydown.enter="tagOpen = false"
            />
            <p v-if="!knownTags.length" class="tag-empty">
              {{ t("effectTags.tagEmpty") }}
            </p>
            <button
              v-for="tag in knownTags"
              :key="tag"
              type="button"
              :class="{ selected: tag === formTag }"
              @click="pickTag(tag)"
            >
              <FIcon :name="tag === formTag ? 'Check' : 'CircleDot'" :size="14" aria-label="" />
              {{ tag }}
            </button>
          </FDropdown>
        </div>
        <label class="tags-field">
          <span>{{ t("effectTags.note") }}</span>
          <textarea
            v-model="formNote"
            rows="2"
            :placeholder="t('effectTags.notePlaceholder')"
          />
        </label>
        <button class="tag-save" type="button" :disabled="saving" @click="save">
          <FIcon name="Plus" :size="13" aria-label="" />
          {{ saving ? t("effectTags.saving") : t("effectTags.save") }}
        </button>
      </div>
    </template>
  </section>
</template>

<style scoped>
.tags-title {
  border-top: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  margin: 14px 0 8px;
  padding-top: 10px;
  text-transform: uppercase;
}
.tags-note {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 4px 0 8px;
}
.tag-item {
  border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  border-radius: 4px;
  margin: 0 0 6px;
  padding: 6px 8px;
}
.tag-item-head {
  align-items: center;
  display: flex;
  gap: 8px;
}
.tag-chip {
  background: var(--accent);
  border-radius: 999px;
  color: var(--foreground);
  flex: none;
  font-size: 11px;
  padding: 2px 9px;
}
.tag-tgi {
  color: var(--muted-foreground);
  flex: 1;
  font: 10px ui-monospace, Consolas, monospace;
  min-width: 0;
  overflow: hidden;
  text-align: end;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tag-remove {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  flex: none;
  min-height: 22px;
  padding: 0 5px;
}
.tag-remove:hover {
  color: var(--danger, #d9534f);
}
.tag-note-text {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 5px 0 0;
  overflow-wrap: anywhere;
}
.tag-form {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: grid;
  gap: 8px;
  padding: 8px;
}
.tags-field {
  display: grid;
  gap: 4px;
  font-size: 12px;
}
.tags-field > span {
  color: var(--muted-foreground);
  font-size: 11px;
}
.tags-field textarea {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  padding: 6px 8px;
  resize: vertical;
}
.tag-trigger {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 8px;
  justify-content: space-between;
  padding: 6px 8px;
  text-align: start;
  width: 100%;
}
.tag-trigger.placeholder .tag-trigger-text {
  color: var(--muted-foreground);
}
.tag-trigger-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tag-input {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  margin-bottom: 4px;
  padding: 6px 8px;
  width: 100%;
}
.tag-empty {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 4px 0;
  padding: 0 9px;
}
.tag-save {
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
  justify-content: center;
  min-height: 26px;
  padding: 0 10px;
}
.tag-save:hover {
  background: var(--accent);
}
.tag-save:disabled {
  cursor: wait;
  opacity: 0.6;
}
</style>
