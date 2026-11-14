<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FMarkdown from "@/components/markdown/FMarkdown.vue";
import FSheet from "@/components/ui/FSheet.vue";
import { isTauri, tauriApi } from "@/api";
import { useModProjectsStore } from "@/stores/modProjects";
import { useToast } from "@/composables/useToast";
import type {
  ModProjectRepoSummary,
  ModProjectView,
} from "@/api/tauri";
import { formatCodeSize } from "../code-support";
import ProjectRepoFileTree from "./ProjectRepoFileTree.vue";
import {
  buildLanguageSegments,
  formatPercent,
} from "./languageBar";

/**
 * Code tab：中央文件树 + 右侧 About/Author/Releases/Languages + 底部 README。
 */
const props = defineProps<{
  project: ModProjectView;
  groupName: string;
}>();

const { t, locale } = useI18n();
const toast = useToast();
const store = useModProjectsStore();

const summary = shallowRef<ModProjectRepoSummary | null>(null);
const summaryError = ref("");
const summaryLoading = ref(false);
const readme = ref("");
const readmeMissing = ref(false);
const readmeLoading = ref(false);

const readmeSheetOpen = ref(false);
const readmeDraft = ref("# README\n\n");
const readmeSaving = ref(false);
const readmeError = ref("");

const aboutSheetOpen = ref(false);
const aboutDraft = ref("");
const aboutSaving = ref(false);
const aboutError = ref("");

const aboutText = computed(
  () =>
    summary.value?.description?.trim() ||
    props.project.description?.trim() ||
    t("studio.repo.aboutEmpty"),
);

const authorText = computed(
  () => summary.value?.author?.trim() || t("studio.repo.authorEmpty"),
);

const languageSegments = computed(() =>
  buildLanguageSegments(
    summary.value?.languages ?? [],
    summary.value?.totalSourceBytes ?? 0,
    t("studio.repo.langOther"),
  ),
);

const dateFormatter = computed(
  () =>
    new Intl.DateTimeFormat(locale.value, {
      month: "short",
      day: "numeric",
      year: "numeric",
    }),
);

const remainingChars = computed(() =>
  Math.max(0, 350 - aboutDraft.value.length),
);

async function loadSummary() {
  if (!isTauri() || !props.project.folderExists) return;
  summaryLoading.value = true;
  summaryError.value = "";
  try {
    summary.value = await tauriApi.studio.projects.repoSummary(
      props.project.relPath,
    );
  } catch (cause) {
    summaryError.value =
      cause && typeof cause === "object" && "message" in cause
        ? String((cause as { message: unknown }).message)
        : String(cause);
    summary.value = null;
  } finally {
    summaryLoading.value = false;
  }
}

async function loadReadme() {
  if (!isTauri() || !props.project.folderExists) {
    readmeMissing.value = true;
    readme.value = "";
    return;
  }
  readmeLoading.value = true;
  readmeMissing.value = false;
  try {
    const doc = await tauriApi.workspace.codeReadText(
      props.project.relPath,
      "README.md",
    );
    readme.value = doc.content;
  } catch {
    try {
      const doc = await tauriApi.workspace.codeReadText(
        props.project.relPath,
        "readme.md",
      );
      readme.value = doc.content;
    } catch {
      readme.value = "";
      readmeMissing.value = true;
    }
  } finally {
    readmeLoading.value = false;
  }
}

function openReadmeSheet() {
  readmeDraft.value = `# ${props.project.name}\n\n`;
  readmeError.value = "";
  readmeSheetOpen.value = true;
}

async function saveReadme() {
  if (readmeSaving.value) return;
  readmeSaving.value = true;
  readmeError.value = "";
  try {
    await tauriApi.workspace.codeWriteText(
      props.project.relPath,
      "README.md",
      readmeDraft.value,
    );
    readmeSheetOpen.value = false;
    toast.success(t("studio.repo.readmeCreated"));
    await loadReadme();
  } catch (cause) {
    readmeError.value =
      cause && typeof cause === "object" && "message" in cause
        ? String((cause as { message: unknown }).message)
        : String(cause);
  } finally {
    readmeSaving.value = false;
  }
}

function openAboutSheet() {
  aboutDraft.value =
    summary.value?.description?.trim() ||
    props.project.description?.trim() ||
    "";
  aboutError.value = "";
  aboutSheetOpen.value = true;
}

async function saveAbout() {
  if (aboutSaving.value) return;
  aboutSaving.value = true;
  aboutError.value = "";
  try {
    await store.updateProject(props.project.id, {
      name: props.project.name,
      groupId: props.project.groupId,
      description: aboutDraft.value.trim() || undefined,
      status: props.project.status,
    });
    aboutSheetOpen.value = false;
    toast.success(t("studio.repo.aboutSaved"));
    await loadSummary();
  } catch (cause) {
    aboutError.value =
      cause && typeof cause === "object" && "message" in cause
        ? String((cause as { message: unknown }).message)
        : String(cause);
  } finally {
    aboutSaving.value = false;
  }
}

watch(
  () => props.project.relPath,
  () => {
    void loadSummary();
    void loadReadme();
  },
  { immediate: true },
);
</script>

<template>
  <div class="code-tab">
    <div class="code-main">
      <ProjectRepoFileTree
        v-if="project.folderExists"
        :project-rel-path="project.relPath"
      />
      <p v-else class="panel-empty">{{ $t("studio.projects.folderMissing") }}</p>

      <article class="readme-card">
        <template v-if="readmeLoading">
          <p class="panel-empty">{{ $t("common.loading") }}</p>
        </template>
        <template v-else-if="readmeMissing">
          <div class="readme-empty">
            <FIcon name="BookOpen" :size="28" aria-label="" />
            <h3>{{ $t("studio.repo.readmeAddTitle") }}</h3>
            <p>{{ $t("studio.repo.readmeAddHint") }}</p>
            <button
              class="readme-add"
              type="button"
              :disabled="!project.folderExists"
              @click="openReadmeSheet"
            >
              {{ $t("studio.repo.readmeAddAction") }}
            </button>
          </div>
        </template>
        <template v-else>
          <header class="readme-head">
            <FIcon name="BookOpen" :size="14" aria-label="" />
            <h3>README</h3>
          </header>
          <FMarkdown :source="readme" class="readme-body" />
        </template>
      </article>
    </div>

    <aside class="code-side">
      <section class="side-block">
        <div class="side-head">
          <h3>{{ $t("studio.repo.about") }}</h3>
          <button
            class="side-gear"
            type="button"
            :title="$t('studio.repo.aboutEdit')"
            :aria-label="$t('studio.repo.aboutEdit')"
            @click="openAboutSheet"
          >
            <FIcon name="Settings" :size="13" aria-label="" />
          </button>
        </div>
        <p class="about-text">{{ aboutText }}</p>
        <ul class="meta-list">
          <li>
            <FIcon name="Tag" :size="12" aria-label="" />
            {{ groupName || $t("studio.projects.filterUngrouped") }}
          </li>
          <li v-if="summary?.version">
            <FIcon name="Hash" :size="12" aria-label="" />
            v{{ summary.version }}
          </li>
          <li v-if="summary?.modType">
            <FIcon name="Boxes" :size="12" aria-label="" />
            {{ summary.modType }}
          </li>
        </ul>
      </section>

      <section class="side-block">
        <h3>{{ $t("studio.repo.author") }}</h3>
        <p class="author-line">
          <FIcon name="UserRound" :size="14" aria-label="" />
          {{ authorText }}
        </p>
      </section>

      <section class="side-block">
        <h3>{{ $t("studio.repo.releases") }}</h3>
        <p v-if="summaryLoading" class="side-empty">{{ $t("common.loading") }}</p>
        <p v-else-if="summaryError" class="side-empty">{{ summaryError }}</p>
        <p v-else-if="!(summary?.releases.length)" class="side-empty">
          {{ $t("studio.repo.releasesEmpty") }}
        </p>
        <ul v-else class="release-list">
          <li
            v-for="release in summary?.releases ?? []"
            :key="release.relativePath"
          >
            <strong>{{ release.name }}</strong>
            <small
              >{{ formatCodeSize(release.size) }} ·
              {{ dateFormatter.format(new Date(release.modifiedMs)) }}</small
            >
            <code>{{ release.buildHash.slice(0, 8) }}</code>
          </li>
        </ul>
      </section>

      <section class="side-block">
        <h3>{{ $t("studio.repo.languages") }}</h3>
        <p v-if="!languageSegments.length" class="side-empty">
          {{ $t("studio.repo.languagesEmpty") }}
        </p>
        <template v-else>
          <div
            class="lang-bar"
            role="img"
            :aria-label="$t('studio.repo.languages')"
          >
            <span
              v-for="segment in languageSegments"
              :key="segment.extension"
              class="lang-seg"
              :style="{
                width: `${Math.max(segment.ratio * 100, 0.5)}%`,
                background: segment.color,
              }"
              :title="`${segment.label} ${formatPercent(segment.ratio)}`"
            ></span>
          </div>
          <ul class="lang-legend">
            <li v-for="segment in languageSegments" :key="segment.extension">
              <i :style="{ background: segment.color }"></i>
              <b>{{ segment.label }}</b>
              <span>{{ formatPercent(segment.ratio) }}</span>
            </li>
          </ul>
          <p v-if="summary?.languageTruncated" class="side-note">
            {{ $t("studio.repo.languagesTruncated") }}
          </p>
        </template>
      </section>
    </aside>

    <FSheet
      v-model:open="readmeSheetOpen"
      :label="$t('studio.repo.readmeAddTitle')"
      width="min(560px, 94vw)"
    >
      <div class="sheet-body">
        <header class="sheet-head">
          <h2>{{ $t("studio.repo.readmeAddTitle") }}</h2>
          <p>{{ $t("studio.repo.readmeAddHint") }}</p>
        </header>
        <label class="sheet-field">
          <span>README.md</span>
          <textarea v-model="readmeDraft" rows="16" spellcheck="false"></textarea>
        </label>
        <p v-if="readmeError" class="sheet-error" role="alert">{{ readmeError }}</p>
        <div class="sheet-actions">
          <button type="button" @click="readmeSheetOpen = false">
            {{ $t("studio.projects.form.cancel") }}
          </button>
          <button
            class="primary"
            type="button"
            :disabled="readmeSaving"
            @click="saveReadme"
          >
            {{
              readmeSaving
                ? $t("common.loading")
                : $t("studio.repo.readmeAddAction")
            }}
          </button>
        </div>
      </div>
    </FSheet>

    <FSheet
      v-model:open="aboutSheetOpen"
      :label="$t('studio.repo.aboutEdit')"
      width="min(480px, 94vw)"
    >
      <div class="sheet-body">
        <header class="sheet-head">
          <h2>{{ $t("studio.repo.aboutEdit") }}</h2>
          <p>{{ $t("studio.repo.aboutEditHint") }}</p>
        </header>
        <label class="sheet-field">
          <span>{{ $t("studio.projects.form.descLabel") }}</span>
          <textarea
            v-model="aboutDraft"
            rows="5"
            maxlength="350"
            :placeholder="$t('studio.projects.form.descPlaceholder')"
          ></textarea>
          <small>{{
            $t("studio.repo.charsRemaining", { n: remainingChars })
          }}</small>
        </label>
        <p v-if="aboutError" class="sheet-error" role="alert">{{ aboutError }}</p>
        <div class="sheet-actions">
          <button type="button" @click="aboutSheetOpen = false">
            {{ $t("studio.projects.form.cancel") }}
          </button>
          <button
            class="primary"
            type="button"
            :disabled="aboutSaving"
            @click="saveAbout"
          >
            {{
              aboutSaving
                ? $t("common.loading")
                : $t("studio.projects.form.save")
            }}
          </button>
        </div>
      </div>
    </FSheet>
  </div>
</template>

<style scoped>
.code-tab {
  align-items: start;
  display: grid;
  gap: 18px;
  grid-template-columns: minmax(0, 1fr) 280px;
}
.code-main {
  display: grid;
  gap: 16px;
  min-width: 0;
}
.code-side {
  display: grid;
  gap: 18px;
}
.side-head {
  align-items: center;
  display: flex;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}
.side-head h3,
.side-block > h3 {
  color: var(--foreground);
  font-size: 13px;
  font-weight: 650;
  margin: 0;
}
.side-block > h3 {
  margin-bottom: 8px;
}
.side-gear {
  align-items: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  padding: 4px;
}
.side-gear:hover {
  border-color: var(--border);
  color: var(--foreground);
}
.about-text,
.author-line,
.side-empty,
.side-note,
.panel-empty {
  color: var(--muted-foreground);
  font-size: 12.5px;
  line-height: 1.5;
  margin: 0;
}
.author-line {
  align-items: center;
  color: var(--foreground);
  display: inline-flex;
  gap: 8px;
}
.meta-list,
.release-list,
.lang-legend {
  display: grid;
  gap: 8px;
  list-style: none;
  margin: 10px 0 0;
  padding: 0;
}
.meta-list li,
.author-line {
  color: var(--muted-foreground);
  font-size: 12px;
}
.meta-list li {
  align-items: center;
  display: flex;
  gap: 7px;
}
.release-list li {
  display: grid;
  gap: 2px;
}
.release-list strong {
  font-size: 12.5px;
}
.release-list small,
.release-list code {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.lang-bar {
  border-radius: 999px;
  display: flex;
  height: 8px;
  overflow: hidden;
  width: 100%;
}
.lang-seg {
  display: block;
  height: 100%;
  min-width: 2px;
}
.lang-legend li {
  align-items: center;
  color: var(--muted-foreground);
  display: grid;
  font-size: 12px;
  gap: 8px;
  grid-template-columns: 10px minmax(0, 1fr) auto;
}
.lang-legend i {
  border-radius: 999px;
  display: block;
  height: 8px;
  width: 8px;
}
.lang-legend b {
  color: var(--foreground);
  font-weight: 550;
  overflow: hidden;
  text-overflow: ellipsis;
}
.readme-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.readme-head {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 8px;
  min-height: 40px;
  padding: 0 14px;
}
.readme-head h3 {
  font-size: 13px;
  font-weight: 650;
  margin: 0;
}
.readme-body {
  padding: 16px 18px 20px;
}
.readme-empty {
  align-items: center;
  color: var(--muted-foreground);
  display: grid;
  gap: 8px;
  justify-items: center;
  padding: 36px 20px;
  text-align: center;
}
.readme-empty h3 {
  color: var(--foreground);
  font-size: 16px;
  margin: 4px 0 0;
}
.readme-empty p {
  font-size: 12.5px;
  margin: 0 0 8px;
  max-width: 320px;
}
.readme-add {
  background: var(--primary);
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--primary-foreground, #fff);
  cursor: pointer;
  font: inherit;
  font-size: 12.5px;
  font-weight: 600;
  min-height: 34px;
  padding: 0 14px;
}
.readme-add:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.sheet-body {
  display: grid;
  gap: 14px;
  padding: 18px;
}
.sheet-head h2 {
  font-size: 16px;
  margin: 0 0 4px;
}
.sheet-head p {
  color: var(--muted-foreground);
  font-size: 12.5px;
  margin: 0;
}
.sheet-field {
  display: grid;
  gap: 6px;
}
.sheet-field > span {
  color: var(--muted-foreground);
  font-size: 11.5px;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.sheet-field textarea {
  background: var(--background);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12.5px;
  line-height: 1.45;
  padding: 10px 12px;
  resize: vertical;
  width: 100%;
}
.sheet-field small {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.sheet-error {
  color: var(--destructive, #f87171);
  font-size: 12.5px;
  margin: 0;
}
.sheet-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.sheet-actions button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  font: inherit;
  font-size: 12.5px;
  min-height: 34px;
  padding: 0 12px;
}
.sheet-actions .primary {
  background: var(--primary);
  border-color: var(--primary);
  color: var(--primary-foreground, #fff);
}
@media (max-width: 980px) {
  .code-tab {
    grid-template-columns: 1fr;
  }
}
</style>
