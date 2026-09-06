<script setup lang="ts">
import FIcon from "@/components/extensions/FIcon.vue";
import type { PropertyPreview } from "@/api/tauri";

defineProps<{ preview: PropertyPreview }>();
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
      <button type="button" disabled>
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
    </div>
    <div class="structured-table-wrap">
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
              <span>{{ entry.name ?? hashLabel(entry.hash) }}</span>
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
.preview-toolbar button {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: not-allowed;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 5px;
  min-height: 28px;
  opacity: 0.6;
  padding: 0 9px;
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
  min-width: 180px;
}
.prop-name span {
  color: var(--foreground);
  display: block;
  font-weight: 600;
  overflow-wrap: anywhere;
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
