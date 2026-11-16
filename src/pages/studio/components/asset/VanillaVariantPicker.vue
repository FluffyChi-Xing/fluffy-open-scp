<script setup lang="ts">
import { shallowRef, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { tauriApi } from "@/api";
import FButton from "@/components/ui/FButton.vue";
import FInput from "@/components/ui/FInput.vue";
import FSheet from "@/components/ui/FSheet.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import VanillaVariantMenuBrowser from "./VanillaVariantMenuBrowser.vue";
import { vanillaMenuBindings } from "./vanillaMenuBindings";

type Match = Awaited<ReturnType<typeof tauriApi.workspace.vanillaVariantLookup>>[number];
const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{
  project: string;
  schemaPath: string;
  inline?: boolean;
  select: (input: { gameData: string; scripts: string; unit: string; lot: string }) => Promise<boolean>;
}>();
const { t } = useI18n();
const appearanceGroup = useId();
const gameData = shallowRef("");
const scripts = shallowRef("");
const query = shallowRef("");
const matches = shallowRef<Match[]>([]);
const busy = shallowRef(false);
const status = shallowRef("");
const mode = shallowRef<"menu" | "tgi">("menu");
const selected = shallowRef<Match>();
const menuSelection = shallowRef("");
const pathsOpen = shallowRef(false);

watch(open, async (visible, _, onCleanup) => {
  if (!visible || gameData.value) return;
  let cancelled = false;
  onCleanup(() => { cancelled = true; });
  try {
    const [manifest, schema, settings] = await Promise.allSettled([
      tauriApi.workspace.codeReadText(props.project, "package.json"),
      tauriApi.workspace.codeReadText(props.project, props.schemaPath),
      tauriApi.settings.get(),
    ]);
    const source = (manifest.status === "fulfilled" ? JSON.parse(manifest.value.content).vanillaVariant : undefined)
      ?? (schema.status === "fulfilled" ? JSON.parse(schema.value.content).vanillaVariant : undefined);
    if (!cancelled) {
      gameData.value = source?.gameData || (settings.status === "fulfilled" ? settings.value.gameDataPath ?? "" : "");
      scripts.value = source?.scripts ?? "";
      pathsOpen.value = !gameData.value.trim();
    }
  } catch {
    if (!cancelled) status.value = "loadFailed";
  }
}, { immediate: true });

function resetMatches() {
  matches.value = [];
  selected.value = undefined;
  status.value = "";
  menuSelection.value = "";
}

function changeMode(value: typeof mode.value) {
  mode.value = value;
  resetMatches();
}

async function browse(directory: boolean) {
  const picked = await openDialog(directory ? { directory: true } : {
    filters: [{ name: t("flow.variant.scripts"), extensions: ["package"] }],
  });
  if (typeof picked !== "string") return;
  if (directory) {
    gameData.value = picked;
    scripts.value = "";
  } else scripts.value = picked;
}

function setMatches(results: Match[]) {
  matches.value = results;
  const first = results[0];
  if (first && results.every((item) => item.unit === first.unit)) selected.value = first;
}

function selectMenu(id: string, label: string) {
  resetMatches();
  menuSelection.value = id;
  const bindings = vanillaMenuBindings(id);
  if (!bindings) {
    status.value = "noMenuBinding";
    return;
  }
  setMatches(bindings.map((binding) => ({ ...binding, name: label || binding.unit.slice(-8) })));
}

async function lookup(tgi: string) {
  if (busy.value || !gameData.value.trim()) return;
  resetMatches();
  if (!/^(?:0x)?[\da-f]{1,8}:(?:0x)?[\da-f]{1,8}:(?:0x)?[\da-f]{1,8}$/i.test(tgi)) {
    status.value = "invalidTgi";
    return;
  }
  busy.value = true;
  try {
    setMatches(await tauriApi.workspace.vanillaVariantLookup({ gameData: gameData.value, scripts: scripts.value, tgi }));
    if (!matches.value.length) status.value = "noResolvedBuilding";
  } catch {
    status.value = "searchFailed";
  } finally {
    busy.value = false;
  }
}

watch([gameData, scripts, query], () => {
  if (mode.value === "tgi") resetMatches();
});

async function confirmSelection() {
  const match = selected.value;
  if (!match || busy.value || !gameData.value.trim()) return;
  busy.value = true;
  try {
    if (await props.select({ gameData: gameData.value, scripts: scripts.value, unit: match.unit, lot: match.lot }))
      open.value = false;
  } catch {
    status.value = "registerFailed";
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <component
    :is="inline ? 'section' : FSheet"
    v-bind="inline ? { class: 'variant-setup', 'aria-label': t('flow.variant.choose') } : {open, label: t('flow.variant.choose'), width: 'min(1120px, 96vw)'}"
    @update:open="open = $event">
    <div class="variant-picker">
      <header class="picker-header">
        <span class="header-icon"><FIcon name="Building2" :size="24" aria-label="" /></span>
        <div class="heading">
          <h2>{{ t("flow.variant.choose") }}</h2>
          <p>{{ t("flow.variant.selectorHint") }}</p>
        </div>
        <FButton variant="ghost" :aria-expanded="pathsOpen" @click="pathsOpen = !pathsOpen">
          <FIcon name="FolderCog" :size="16" aria-label="" />
          {{ t("flow.variant.sourceSettings") }}
        </FButton>
      </header>

      <div v-if="pathsOpen" class="paths-panel">
        <div v-for="directory in [true, false]" :key="String(directory)" class="path-field">
          <label>
            {{ t(directory ? "flow.variant.gameData" : "flow.variant.scripts") }}
            <FInput v-if="directory" v-model="gameData" :disabled="busy" spellcheck="false" />
            <FInput v-else v-model="scripts" :disabled="busy" spellcheck="false" :placeholder="t('flow.variant.scriptsAuto')" />
          </label>
          <FButton
            variant="secondary"
            :disabled="busy"
            :title="t(directory ? 'flow.variant.chooseDirectory' : 'flow.variant.chooseScripts')"
            :aria-label="t(directory ? 'flow.variant.chooseDirectory' : 'flow.variant.chooseScripts')"
            @click="browse(directory)">
            <FIcon name="FolderOpen" :size="17" aria-label="" />
          </FButton>
        </div>
      </div>

      <div class="picker-grid">
        <section class="lookup-panel">
          <div class="mode-switch" role="group" :aria-label="t('flow.variant.choose')">
            <button type="button" :disabled="busy" :aria-pressed="mode === 'menu'" @click="changeMode('menu')">
              <FIcon name="LayoutList" :size="16" aria-label="" />{{ t("flow.variant.menuLookup") }}
            </button>
            <button type="button" :disabled="busy" :aria-pressed="mode === 'tgi'" @click="changeMode('tgi')">
              <FIcon name="ScanLine" :size="16" aria-label="" />{{ t("flow.variant.tgiLookup") }}
            </button>
          </div>
          <VanillaVariantMenuBrowser
            v-if="mode === 'menu'"
            :disabled="busy"
            :selected-id="menuSelection"
            @lookup="selectMenu" />
          <form v-else class="tgi-form" @submit.prevent="lookup(query.trim())">
            <label>
              {{ t("flow.variant.tgiInput") }}
              <FInput
                v-model="query"
                class="tgi-value"
                maxlength="40"
                spellcheck="false"
                :disabled="busy"
                :invalid="status === 'invalidTgi'"
                placeholder="00B1B104:40B9A133:14FF6F1D" />
            </label>
            <FButton type="submit" :disabled="busy || !query.trim() || !gameData.trim()">
              <FIcon name="Search" :size="16" aria-label="" />{{ t("flow.variant.resolve") }}
            </FButton>
          </form>
        </section>

        <aside class="choices">
          <div class="choice-header">
            <FIcon name="Layers" :size="18" aria-label="" />
            <h3>{{ t("flow.variant.chooseAppearance") }}</h3>
          </div>
          <p v-if="busy || status" class="choice-message" role="status">
            {{ busy ? t("common.loading") : t(`flow.variant.${status}`) }}
          </p>
          <template v-if="matches.length">
            <fieldset class="appearances" :aria-label="t('flow.variant.chooseAppearance')" :disabled="busy">
              <label v-for="match in matches" :key="`${match.unit}/${match.lot}`" :class="{ selected: selected === match }">
                <input type="radio" :name="appearanceGroup" :checked="selected === match" @change="selected = match" />
                <span class="appearance-name">
                  {{ match.name }}
                  <code>{{ match.lot }}</code>
                </span>
                <small v-if="match.matched || match.primary">{{ t(match.matched ? "flow.variant.matchedAppearance" : "flow.variant.primaryAppearance") }}</small>
              </label>
            </fieldset>
            <div class="choice-footer">
              <p v-if="selected">{{ t("flow.variant.resolvedUnit") }} <code>{{ selected.unit }}</code></p>
              <FButton v-if="!gameData.trim()" variant="secondary" @click="pathsOpen = true">
                <FIcon name="FolderOpen" :size="16" aria-label="" />{{ t("flow.variant.setGameData") }}
              </FButton>
              <FButton v-else :disabled="!selected || busy" @click="confirmSelection">
                {{ t("flow.variant.startDevelopment") }}<FIcon name="ArrowRight" :size="16" aria-label="" />
              </FButton>
            </div>
          </template>
          <div v-else-if="!busy && !status" class="choice-empty">
            <span><FIcon name="Box" :size="27" aria-label="" /></span>
            <p>{{ t("flow.variant.selectionEmpty") }}</p>
          </div>
        </aside>
      </div>
    </div>
  </component>
</template>

<style scoped>
.variant-setup { flex: 1; min-height: 0; overflow: auto; }
.variant-picker { container-type: inline-size; display: grid; gap: 16px; padding: clamp(12px, 2.4vw, 32px); margin-inline: auto; max-width: 1320px; }
.picker-header { display: flex; align-items: center; flex-wrap: wrap; gap: 12px; }
.header-icon, .choice-empty > span { display: grid; place-items: center; inline-size: 48px; block-size: 48px; flex: none; border-radius: var(--radius-xl); background: var(--accent); color: var(--accent-foreground); }
.heading { flex: 1; min-inline-size: min(100%, 15rem); }
h2 { font-size: clamp(20px, 2vw, 27px); line-height: 1.25; font-weight: 650; }
.heading p, .choice-empty p, .choice-message { color: var(--muted-foreground); font-size: 13px; line-height: 1.55; }
.heading p { margin-block-start: 3px; }
.paths-panel { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; padding: 12px; border: 1px solid var(--border); border-radius: var(--radius-xl); background: var(--surface); }
.path-field { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: end; gap: 8px; }
.path-field label, .tgi-form label { display: grid; gap: 6px; color: var(--muted-foreground); font-size: 13px; }
.picker-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(235px, 28%); align-items: start; gap: 14px; }
.lookup-panel, .choices { min-width: 0; border: 1px solid var(--border); border-radius: var(--radius-xl); background: var(--surface); box-shadow: var(--shadow-sm); }
.mode-switch { display: flex; gap: 4px; padding: 8px; border-bottom: 1px solid var(--border); }
.mode-switch button { display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-height: 42px; border-radius: var(--radius-md); padding: 0 12px; color: var(--muted-foreground); font-size: 13px; font-weight: 600; transition: background-color 140ms ease, color 140ms ease, scale 100ms ease; }
.mode-switch button:hover:not(:disabled) { background: var(--surface-hover); color: var(--foreground); }
.mode-switch button[aria-pressed="true"] { background: var(--accent); color: var(--accent-foreground); }
.mode-switch button:active:not(:disabled) { scale: 0.96; }
.mode-switch button:disabled { opacity: .55; }
.mode-switch button:focus-visible { outline: 2px solid var(--ring); outline-offset: -2px; }
.tgi-form { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: end; gap: 10px; padding: 18px; }
.tgi-value, code { font-family: ui-monospace, monospace; }
.choices { display: grid; gap: 12px; padding: 14px; }
.choice-header { display: flex; align-items: center; gap: 8px; color: var(--accent-foreground); }
.choice-header h3 { color: var(--foreground); font-size: 14px; font-weight: 650; }
.choice-empty { display: grid; min-height: 135px; align-content: center; justify-items: center; gap: 12px; padding: 12px 4px; text-align: center; }
.choice-empty > span { background: var(--surface-hover); color: var(--muted-foreground); }
.choice-message { margin-block: 4px; }
.appearances { display: grid; gap: 4px; max-height: 16rem; overflow: auto; min-inline-size: 0; padding: 2px; }
.appearances label { display: flex; align-items: center; gap: 8px; padding: 8px; border-radius: var(--radius-md); font-size: 13px; cursor: pointer; }
.appearances label:hover { background: var(--surface-hover); }
.appearances label.selected { background: var(--accent); }
.appearances input { flex: none; accent-color: var(--primary); }
.appearance-name { min-width: 0; }
code { display: block; color: var(--muted-foreground); overflow-wrap: anywhere; font-size: 11px; }
.appearances small { margin-inline-start: auto; color: var(--accent-foreground); font-size: 11px; }
.choice-footer { display: grid; gap: 10px; border-top: 1px solid var(--border); padding-top: 12px; font-size: 12px; }
@media (prefers-reduced-motion: reduce) { .mode-switch button { transition: none; } }
@container (max-width: 790px) {
  .picker-grid, .paths-panel { grid-template-columns: minmax(0, 1fr); }
  .choice-empty { min-height: 80px; }
}
@container (max-width: 400px) {
  .tgi-form { grid-template-columns: minmax(0, 1fr); }
  .mode-switch button { flex: 1; padding-inline: 4px; }
}
</style>
