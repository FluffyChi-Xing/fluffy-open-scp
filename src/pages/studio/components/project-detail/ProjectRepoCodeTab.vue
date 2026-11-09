<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FMarkdown from "@/components/markdown/FMarkdown.vue";
import { isTauri, tauriApi } from "@/api";
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

const summary = shallowRef<ModProjectRepoSummary | null>(null);
const summaryError = ref("");
const summaryLoading = ref(false);
const readme = ref("");
const readmeMissing = ref(false);
const readmeLoading = ref(false);

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
        <header class="readme-head">
          <FIcon name="BookOpen" :size="14" aria-label="" />
          <h3>README</h3>
        </header>
        <p v-if="readmeLoading" class="panel-empty">{{ $t("common.loading") }}</p>
        <p v-else-if="readmeMissing" class="panel-empty">
          {{ $t("studio.repo.readmeMissing") }}
        </p>
        <FMarkdown v-else :source="readme" class="readme-body" />
      </article>
    </div>

    <aside class="code-side">
      <section class="side-block">
        <h3>{{ $t("studio.repo.about") }}</h3>
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
        <p
          v-else-if="!(summary?.releases.length)"
          class="side-empty"
        >
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
        <p
          v-if="!languageSegments.length"
          class="side-empty"
        >
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
  </div>
</template>

<style scoped>
.code-tab {
  display: grid;
  gap: 18px;
  grid-template-columns: minmax(0, 1fr) 280px;
  align-items: start;
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
.side-block h3 {
  color: var(--foreground);
  font-size: 13px;
  font-weight: 650;
  margin: 0 0 8px;
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
@media (max-width: 980px) {
  .code-tab {
    grid-template-columns: 1fr;
  }
}
</style>
