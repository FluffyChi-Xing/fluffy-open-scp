<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useMenuImages } from "../asset/useMenuImages";
import type { RoadMenuSettings, RoadSchema } from "./roadSchema";

const props = defineProps<{
  project: string;
  schema: RoadSchema;
  saving: boolean;
}>();
const emit = defineEmits<{
  update: [patch: Partial<Pick<RoadSchema, "title" | "description" | "menu">>];
}>();
const { t } = useI18n();

function updateMenu(patch: RoadMenuSettings) {
  emit("update", { menu: { ...props.schema.menu, ...patch } });
}

const { previews, importingImage, uploadMenuImage } = useMenuImages(
  () => props.project,
  () => ({
    iconPng: props.schema.menu?.iconPng ?? "",
    marqueePng: props.schema.menu?.marqueePng ?? "",
  }),
  ({ iconPng, marqueePng }) => {
    if (iconPng !== undefined) updateMenu({ iconPng });
    if (marqueePng !== undefined) updateMenu({ marqueePng });
  },
  true,
);
const images = computed(() =>
  (
    [
      { kind: "icon", field: "iconPng", label: "menuIcon" },
      { kind: "marquee", field: "marqueePng", label: "menuMarquee" },
    ] as const
  ).map((image) => ({ ...image, path: props.schema.menu?.[image.field] })),
);

function setPosition(event: Event) {
  const input = event.currentTarget as HTMLInputElement;
  const value = input.valueAsNumber;
  if (!input.value && !input.validity.badInput) updateMenu({ position: null });
  else if (
    Number.isInteger(value) &&
    value >= -2147483648 &&
    value <= 2147483647
  ) {
    updateMenu({ position: value });
  } else {
    input.value = String(props.schema.menu?.position ?? "");
  }
}
</script>

<template>
  <div class="menu-grid">
    <section class="card">
      <h3>{{ t("roadAsset.project.menuEntry") }}</h3>
      <p>{{ t("roadAsset.project.nativeCategory") }}</p>
      <label
        >{{ t("flow.variant.title") }}
        <input
          :value="schema.title"
          :disabled="saving"
          @input="
            emit('update', { title: ($event.target as HTMLInputElement).value })
          "
        />
      </label>
      <label
        >{{ t("flow.description") }}
        <textarea
          :value="schema.description"
          :disabled="saving"
          rows="3"
          @input="
            emit('update', {
              description: ($event.target as HTMLTextAreaElement).value,
            })
          "
        />
      </label>
      <label
        >{{ t("roadAsset.project.position") }}
        <input
          type="number"
          min="-2147483648"
          max="2147483647"
          step="1"
          :value="schema.menu?.position ?? ''"
          :disabled="saving"
          :placeholder="t('roadAsset.project.donorDefault')"
          @change="setPosition"
        />
      </label>
    </section>
    <div class="artwork">
      <section v-for="image in images" :key="image.kind" class="card">
        <h3>{{ t(`studio.assetWorkbench.${image.label}`) }}</h3>
        <img v-if="previews[image.kind]" :src="previews[image.kind]" alt="" />
        <code v-if="image.path">{{ image.path }}</code>
        <p v-else>{{ t("roadAsset.project.donorDefault") }}</p>
        <div class="actions">
          <button
            type="button"
            :disabled="saving || importingImage"
            @click="uploadMenuImage(image.kind)"
          >
            {{ t("flow.variant.replaceArtwork") }}
          </button>
          <button
            v-if="image.path"
            type="button"
            :disabled="saving || importingImage"
            @click="updateMenu({ [image.field]: '' })"
          >
            {{ t("flow.variant.clearArtwork") }}
          </button>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.menu-grid,
.card,
label {
  display: grid;
  gap: 8px;
  min-inline-size: 0;
}
.card {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 12px;
}
.artwork {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 205px), 1fr));
}
h3 {
  font-weight: 600;
}
p,
code {
  color: var(--muted-foreground);
  overflow-wrap: anywhere;
}
input,
textarea,
button {
  background: var(--surface);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 5px 8px;
}
textarea {
  resize: vertical;
}
img {
  inline-size: 100%;
  block-size: 90px;
  object-fit: contain;
}
.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}
</style>
