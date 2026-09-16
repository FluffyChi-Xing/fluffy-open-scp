<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import type { PropertyPreview } from "@/api/tauri";
import { isDecalAtlasGroup } from "@/lib/resource-types";
import PropertyEditor from "../property-editor/PropertyEditor.vue";
import DecalDictionaryGallery from "./DecalDictionaryGallery.vue";

const props = defineProps<{ preview: PropertyPreview }>();
const editorOpen = ref(false);
const { locale } = useI18n();

/** 后端 semantic 判定的资源子类型徽章（按 locale 选用后端下发的标签）。 */
const semanticLabel = computed(() => {
  const tag = props.preview.semantic;
  if (!tag) return null;
  return locale.value.startsWith("zh") ? tag.labelZh : tag.labelEn;
});

/** Decal Dictionary（贴花图鉴）用相册取代属性表。 */
const isDecalDictionary = computed(() =>
  isDecalAtlasGroup(props.preview.tgi.group),
);
const view = ref<"gallery" | "table">("gallery");
watch(
  isDecalDictionary,
  (value) => {
    view.value = value ? "gallery" : "table";
  },
  { immediate: true },
);

function hashLabel(hash: number) {
  return `0x${hash.toString(16).padStart(8, "0").toUpperCase()}`;
}
</script>

<template>
  <div class="structured-preview">
    <div class="preview-toolbar" role="toolbar" aria-label="Property toolbar">
      <button type="button" disabled>
        <FIcon name="Plus" :size="13" aria-label="" />{{
          $t("package.toolbarAddProperty")
        }}
      </button>
      <button type="button" disabled>
        <FIcon name="Pencil" :size="13" aria-label="" />{{
          $t("package.toolbarEdit")
        }}
      </button>
      <button type="button" disabled>
        <FIcon name="ListTree" :size="13" aria-label="" />{{
          $t("package.toolbarViewChildren")
        }}
      </button>
      <button
        type="button"
        :aria-label="$t('package.propertyEditor')"
        @click="editorOpen = true"
      >
        <FIcon name="SquarePen" :size="13" aria-label="" />{{
          $t("package.toolbarAdvancedEditors")
        }}
      </button>
      <button type="button" disabled>
        <FIcon name="Wrench" :size="13" aria-label="" />{{
          $t("package.toolbarTools")
        }}
        <FIcon name="ChevronDown" :size="11" aria-label="" />
      </button>
      <button
        v-if="isDecalDictionary"
        type="button"
        :aria-pressed="view === 'gallery'"
        @click="view = view === 'gallery' ? 'table' : 'gallery'"
      >
        <FIcon :name="view === 'gallery' ? 'ListTree' : 'Image'" :size="13" aria-label="" />{{
          view === "gallery" ? $t("decal.switchTable") : $t("decal.switchGallery")
        }}
      </button>
      <span v-if="semanticLabel" class="semantic-badge">{{ semanticLabel }}</span>
    </div>
    <PropertyEditor
      v-model:open="editorOpen"
      :package-id="preview.packageId"
      :tgi="preview.tgi"
    />
    <DecalDictionaryGallery
      v-if="isDecalDictionary && view === 'gallery'"
      :package-id="preview.packageId"
      :tgi="preview.tgi"
    />
    <div v-else class="structured-table-wrap">
      <table class="structured-table">
        <thead>
          <tr>
            <th>{{ $t("package.propertyColumn") }}</th>
            <th>{{ $t("package.valueColumn") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="entry in preview.entries" :key="entry.hash">
            <td class="prop-name">
              <span :title="entry.name ?? hashLabel(entry.hash)">{{
                entry.name ?? hashLabel(entry.hash)
              }}</span>
              <small
                >{{ entry.typeName
                }}<template v-if="entry.arrayLen !== null"
                  >[{{ entry.arrayLen }}]</template
                ></small
              >
            </td>
            <td class="prop-value">
              {{ entry.value || (entry.arrayLen === null ? "—" : "") }}
            </td>
          </tr>
        </tbody>
      </table>
      <p v-if="!preview.entries.length" class="empty-hint">
        {{ $t("package.noPreview") }}
      </p>
    </div>
  </div>
</template>

<style scoped>
.structured-preview {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
}
.preview-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.semantic-badge {
  align-self: center;
  background: var(--surface-hover);
  border-radius: 4px;
  color: var(--muted-foreground);
  font-size: 11px;
  margin-left: auto;
  padding: 1px 6px;
  white-space: nowrap;
}
.preview-toolbar button {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 5px;
  min-height: 28px;
  padding: 0 9px;
}
.preview-toolbar button:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--foreground);
}
.preview-toolbar button:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.structured-table-wrap {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  max-height: 420px;
  overflow: auto;
  min-height: 240px;
}
.structured-table {
  border-collapse: collapse;
  font-size: 12px;
  min-width: 100%;
  text-align: start;
}
.structured-table th {
  border-bottom: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  padding: 9px 10px;
  position: sticky;
  text-transform: uppercase;
  top: 0;
  background: var(--surface-elevated);
  text-align: start;
}
.structured-table td {
  border-top: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
  padding: 8px 10px;
  vertical-align: top;
}
.prop-name {
  max-width: 320px;
  min-width: 180px;
}
.prop-name span {
  color: var(--foreground);
  display: block;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.prop-name small {
  color: var(--subtle-foreground);
  font-size: 10px;
}
.prop-value {
  color: var(--muted-foreground);
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}
.empty-hint {
  color: var(--subtle-foreground);
  font-size: 12px;
  padding: 26px 12px;
  text-align: center;
}
</style>
