<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FButton from "@/components/ui/FButton.vue";
import FInput from "@/components/ui/FInput.vue";
import { gameAssetTgiDataUrl } from "@/lib/game-assets";
import { loadReplicaCatalogue, loadReplicaToolPreview } from "@/lib/game-ui/replica/loader";
import { useMenuImages } from "./useMenuImages";
import { vanillaMenuIdsForUnit } from "./vanillaMenuBindings";

type MenuFields = { title: string; description?: string; iconPng?: string; marqueePng?: string };
const props = defineProps<{ project: string; unit: string; menu: MenuFields; disabled: boolean }>();
const emit = defineEmits<{ update: [patch: Partial<MenuFields>] }>();
const { t } = useI18n();
const original = shallowRef<{ name: string; desc?: string; preview?: string | null; marquee?: string | null }>();
const artwork = shallowRef<{ icon?: string; marquee?: string }>({});
const loading = shallowRef(false);
const { previews, importingImage, uploadMenuImage } = useMenuImages(
  () => props.project,
  () => ({ iconPng: props.menu.iconPng ?? "", marqueePng: props.menu.marqueePng ?? "" }),
  (patch) => emit("update", patch),
);

watch(() => props.unit, async (unit, _, onCleanup) => {
  let current = true;
  onCleanup(() => { current = false; });
  original.value = undefined;
  artwork.value = {};
  loading.value = true;
  try {
    const ids = new Set(vanillaMenuIdsForUnit(unit));
    if (!ids.size) return;
    const data = await loadReplicaCatalogue();
    const tool = Object.values(data.tools).flat().find((item) => ids.has(item.instance));
    const module = Object.values(data.submenus ?? {}).flat().find((item) => ids.has(item.instance));
    const source = tool ? { ...tool, name: tool.label } : module;
    if (!current || !source) return;
    original.value = source;
    const marquee = /tool_([0-9a-f]{8})\.jpg$/i.exec(source.marquee ?? "");
    const [icon, background] = await Promise.all([
      source.preview ? loadReplicaToolPreview(source.preview) : undefined,
      marquee ? gameAssetTgiDataUrl(0x3f8662ea, Number.parseInt(marquee[1], 16), 0)
        : undefined,
    ]);
    if (current) artwork.value = { icon, marquee: background ?? source.marquee ?? undefined };
  } catch {
    // The variant stays editable when the original menu catalogue is unavailable.
  } finally {
    if (current) loading.value = false;
  }
}, { immediate: true });

const images = computed(() => ([
  { kind: "icon", field: "iconPng", label: "menuIcon" },
  { kind: "marquee", field: "marqueePng", label: "menuMarquee" },
] as const).map((item) => ({
  ...item,
  original: artwork.value[item.kind],
  override: previews[item.kind],
})));
</script>

<template>
  <div class="variant-menu">
    <section class="original-menu">
      <h3>{{ t("flow.variant.originalMenu") }}</h3>
      <template v-if="original">
        <strong>{{ original.name }}</strong>
        <p v-if="original.desc">{{ original.desc }}</p>
        <FButton
          v-if="original.name" variant="secondary" :disabled="disabled"
          @click="emit('update', { title: original.name, description: original.desc ?? '' })">
          {{ t("flow.variant.useOriginalMenuText") }}
        </FButton>
      </template>
      <p v-else>{{ t(loading ? "common.loading" : "flow.variant.originalMenuUnavailable") }}</p>
    </section>
    <div class="menu-text">
      <label>{{ t("flow.variant.title") }}
        <FInput :model-value="menu.title" :disabled="disabled" @update:model-value="emit('update', { title: $event })" />
      </label>
      <label>{{ t("flow.description") }}
        <textarea
          :value="menu.description ?? ''" :disabled="disabled" rows="3"
          @input="emit('update', { description: ($event.target as HTMLTextAreaElement).value })" />
      </label>
    </div>
    <div class="menu-art">
      <section v-for="item in images" :key="item.kind" class="art-card">
        <h3>{{ t(`studio.assetWorkbench.${item.label}`) }}</h3>
        <div class="art-previews">
          <figure v-if="item.original">
            <img :src="item.original" alt="" @error="artwork = { ...artwork, [item.kind]: undefined }" />
            <figcaption>{{ t("flow.variant.originalArtwork") }}</figcaption>
          </figure>
          <p v-else-if="!loading && original" class="art-unavailable">{{ t("flow.variant.originalArtworkUnavailable") }}</p>
          <figure v-if="item.override">
            <img :src="item.override" alt="" />
            <figcaption>{{ t("flow.variant.variantArtwork") }}</figcaption>
          </figure>
        </div>
        <div class="art-actions">
          <FButton variant="secondary" :disabled="disabled || importingImage" @click="uploadMenuImage(item.kind)">
            {{ t("flow.variant.replaceArtwork") }}
          </FButton>
          <FButton v-if="menu[item.field]" variant="ghost" :disabled="disabled || importingImage" @click="emit('update', { [item.field]: '' })">
            {{ t("flow.variant.clearArtwork") }}
          </FButton>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.variant-menu, .original-menu, .menu-text, .art-card { display: grid; min-width: 0; gap: 10px; }
.original-menu, .art-card { padding: 12px; border: 1px solid var(--border); border-radius: var(--radius-md); background: var(--surface-elevated); }
.original-menu p, figcaption, .art-unavailable { color: var(--muted-foreground); font-size: 12px; }
.original-menu :deep(button) { justify-self: start; }
h3 { font-size: 13px; font-weight: 600; }
label { display: grid; gap: 5px; font-size: 12px; }
textarea { min-width: 0; padding: 8px; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface-elevated); resize: vertical; }
.menu-art { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(220px, 100%), 1fr)); gap: 10px; }
.art-previews, .art-actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: end; }
figure { min-width: 0; flex: 1 1 95px; margin: 0; }
img { width: 100%; height: 86px; object-fit: contain; border-radius: var(--radius-sm); background: var(--surface); }
</style>
