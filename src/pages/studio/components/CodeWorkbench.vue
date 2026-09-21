<script setup lang="ts">
import { computed, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FCode from "@/components/ui/FCode.vue";
import FPopover from "@/components/ui/FPopover.vue";
import {
  Resizable,
  ResizableHandle,
  ResizablePanel,
} from "@/components/ui/resizable";
import { isTauri, tauriApi } from "@/api";
import { useToast } from "@/composables/useToast";
import {
  classifyCodeFile,
  extensionOf,
  flattenCodeTree,
  formatCodeSize,
  joinCodePath,
  shikiLanguageOf,
  type CodeRow,
  type CodeTreeNodeDto,
} from "./code-support";
import type {
  CodePackageEntry,
  CodeResourcePreview,
  CodeTextDocument,
  CodeTreeResponse,
  ModProjectView,
} from "@/api/tauri";
import { rgbaBase64ToPngDataUrl, makeCheckerboard } from "@/lib/raster-editor/encoders";
import NotesSheet from "./NotesSheet.vue";

/**
 * Code 工作台（M-CM1 文件浏览 + M-CM2 解析预览）：
 * 开发工作台模组列表「开发」按钮展开的全屏 sheet 内容，root = 该项目的
 * 文件夹（modRoot/<relPath>，后端同套越界/符号链接防护）。
 * 左侧 = 项目目录树；右侧 = 按扩展名分流的查看器 + 右上角 toolbar：
 * 文本族 FCode 高亮（bat/lua/json/xml…shiki 映射），图片带缩放档位，
 * .package 分裂为「资源列表 | 资源预览」双 Resizable 区，more 浮层收纳
 * 文件元信息（含 DBPF 统计），笔记按钮打开文档工作区的模组笔记 sheet。
 * 打开时 package.json 缺失则自动扫描生成（manifest）。
 */
const props = defineProps<{ project: ModProjectView }>();
const emit = defineEmits<{ close: [] }>();

const { t, locale } = useI18n();
const toast = useToast();

const tree = shallowRef<CodeTreeResponse | null>(null);
const loading = ref(false);
/** 阻断性错误（未配置/不可访问），其余错误走 toast。 */
const fatalCode = ref<string | null>(null);
const expanded = ref(new Set<string>());
/** 首层目录只自动展开一次；此后尊重用户的折叠操作。 */
const autoExpanded = ref(false);
const selected = shallowRef<CodeTreeNodeDto | null>(null);
const viewerLoading = ref(false);
const textDoc = shallowRef<CodeTextDocument | null>(null);
const imageSrc = ref("");
const imageDims = ref<{ width: number; height: number } | null>(null);
const packageInfo = shallowRef<{
  entryCount: number;
  decompressedTotal: number;
  size: number;
  types: { typeId: number; name: string; count: number }[];
} | null>(null);
const viewerError = ref("");
/** package 双栏：资源列表与选中资源的只读预览。 */
const packageEntries = shallowRef<CodePackageEntry[]>([]);
const entriesLoading = ref(false);
const selectedEntry = shallowRef<CodePackageEntry | null>(null);
const resourcePreview = shallowRef<CodeResourcePreview | null>(null);
const resourceLoading = ref(false);
/** 资源列表筛选：类型分类标签 + TGI 搜索（instance/group hex 包含匹配）。 */
const entryTypeFilter = ref<number | null>(null);
const entrySearch = ref("");
const filteredEntries = computed(() => {
  const query = entrySearch.value.trim().toLowerCase();
  return packageEntries.value.filter((entry) => {
    if (
      entryTypeFilter.value !== null &&
      entry.typeId !== entryTypeFilter.value
    ) {
      return false;
    }
    if (!query) return true;
    return entry.instanceId
      .toString(16)
      .includes(query.replace(/^0x/, ""));
  });
});
const semanticLabel = (entry: CodePackageEntry): string => {
  if (!entry.semantic) return "";
  return locale.value.startsWith("zh")
    ? entry.semantic.labelZh
    : entry.semantic.labelEn;
};
/** 图片缩放档位。 */
type ImageZoom = "fit" | 1 | 2 | 4;
const imageZoom = ref<ImageZoom>("fit");
/** manifest（package.json）机制：缺失自动生成。 */
const manifestCreated = ref<boolean | null>(null);
const moreOpen = ref(false);
const notesOpen = ref(false);

const fatalMessage = computed(() => {
  switch (fatalCode.value) {
    case "mod_root_not_configured":
      return t("studio.code.needModRoot");
    case "mod_root_unavailable":
      return t("studio.code.rootUnavailable");
    default:
      return null;
  }
});

const rows = computed(() =>
  flattenCodeTree(tree.value?.entries ?? [], expanded.value),
);

async function loadTree(keepSelection = false) {
  if (!isTauri() || loading.value) return;
  loading.value = true;
  fatalCode.value = null;
  try {
    const response = await tauriApi.workspace.codeTree(props.project.relPath);
    tree.value = response;
    // 首层目录自动展开一次：模组内子文件夹（如 SimCityData）直接可见。
    if (!autoExpanded.value) {
      autoExpanded.value = true;
      expanded.value = new Set(
        response.entries
          .filter((entry) => entry.kind === "folder")
          .map((entry) => entry.relativePath),
      );
    }
    if (!keepSelection || selected.value) {
      const stillThere =
        selected.value &&
        findNode(response.entries, selected.value.relativePath);
      if (!stillThere) clearViewer();
    }
  } catch (cause) {
    const code = (cause as { code?: string }).code ?? "command_failed";
    if (
      code === "mod_root_not_configured" ||
      code === "mod_root_unavailable"
    ) {
      fatalCode.value = code;
    } else {
      toast.error(t("studio.code.loadFailed"));
    }
  } finally {
    loading.value = false;
  }
  // package.json 机制：缺失即扫描自动生成（结果以文件形式出现在树里）。
  try {
    const manifest = await tauriApi.workspace.codeManifest(props.project.relPath);
    if (manifest.created) {
      manifestCreated.value = true;
      toast.success(t("studio.code.manifestCreated"));
      const response = await tauriApi.workspace.codeTree(props.project.relPath);
      tree.value = response;
    }
  } catch {
    /* 清单生成失败不阻断浏览 */
  }
}

function findNode(
  entries: CodeTreeNodeDto[],
  relativePath: string,
): CodeTreeNodeDto | null {
  for (const node of entries) {
    if (node.relativePath === relativePath) return node;
    if (node.kind === "folder") {
      const found = findNode(node.children, relativePath);
      if (found) return found;
    }
  }
  return null;
}

function clearViewer() {
  selected.value = null;
  textDoc.value = null;
  imageSrc.value = "";
  imageDims.value = null;
  packageInfo.value = null;
  packageEntries.value = [];
  selectedEntry.value = null;
  resourcePreview.value = null;
  viewerError.value = "";
  imageZoom.value = "fit";
}

function rowIcon(row: CodeRow): string {
  if (row.kind === "folder") return expanded.value.has(row.relativePath) ? "FolderOpen" : "boxes";
  switch (classifyCodeFile(row.name)) {
    case "package":
      return "Package";
    case "image":
      return "Image";
    case "text":
      return "FileText";
    default:
      return "Box";
  }
}

function onRowClick(row: CodeRow) {
  if (row.kind === "folder") {
    const next = new Set(expanded.value);
    if (next.has(row.relativePath)) {
      next.delete(row.relativePath);
    } else {
      next.add(row.relativePath);
    }
    expanded.value = next;
    return;
  }
  const node = findNode(tree.value?.entries ?? [], row.relativePath);
  if (node) void selectFile(node);
}

async function selectFile(node: CodeTreeNodeDto) {
  if (viewerLoading.value || !tree.value) return;
  selected.value = node;
  textDoc.value = null;
  imageSrc.value = "";
  imageDims.value = null;
  packageInfo.value = null;
  packageEntries.value = [];
  selectedEntry.value = null;
  resourcePreview.value = null;
  viewerError.value = "";
  imageZoom.value = "fit";
  entryTypeFilter.value = null;
  entrySearch.value = "";
  viewerLoading.value = true;
  try {
    const kind = classifyCodeFile(node.name);
    if (kind === "text") {
      textDoc.value = await tauriApi.workspace.codeReadText(
        props.project.relPath,
        node.relativePath,
      );
    } else if (kind === "image") {
      const absolute = joinCodePath(tree.value.rootPath, node.relativePath);
      const image = await tauriApi.raster.readImageRgba(absolute);
      imageDims.value = { width: image.width, height: image.height };
      imageSrc.value = rgbaBase64ToPngDataUrl(
        image.rgbaBase64,
        image.width,
        image.height,
      );
    } else if (kind === "package") {
      const [info, entries] = await Promise.all([
        tauriApi.workspace.codePackageInfo(props.project.relPath, node.relativePath),
        tauriApi.workspace.codePackageEntries(props.project.relPath, node.relativePath),
      ]);
      packageInfo.value = {
        entryCount: info.entryCount,
        decompressedTotal: info.decompressedTotal,
        size: info.size,
        types: info.types,
      };
      packageEntries.value = entries;
    }
  } catch (cause) {
    const code = (cause as { code?: string }).code ?? "";
    viewerError.value =
      code === "limit_exceeded"
        ? t("studio.code.textTooLarge")
        : t("studio.code.loadFailed");
  } finally {
    viewerLoading.value = false;
  }
}

async function selectEntry(entry: CodePackageEntry) {
  if (!selected.value || resourceLoading.value) return;
  selectedEntry.value = entry;
  resourcePreview.value = null;
  resourceLoading.value = true;
  try {
    resourcePreview.value = await tauriApi.workspace.codeResourcePreview(
      props.project.relPath,
      selected.value.relativePath,
      entry.typeId,
      entry.groupId,
      entry.instanceId,
    );
  } catch {
    toast.error(t("studio.code.loadFailed"));
  } finally {
    resourceLoading.value = false;
  }
}

const viewerKind = computed(() =>
  selected.value ? classifyCodeFile(selected.value.name) : null,
);

const checkerboardUrl = makeCheckerboard();

const isJson = computed(
  () => extensionOf(selected.value?.name ?? "") === "json",
);

/** JSON 尝试美化（解析失败原样展示）。 */
const textContent = computed(() => {
  const document = textDoc.value;
  if (!document) return "";
  if (!isJson.value) return document.content;
  try {
    return JSON.stringify(JSON.parse(document.content), null, 2);
  } catch {
    return document.content;
  }
});

const textLanguage = computed(() => shikiLanguageOf(selected.value?.name ?? ""));

const IMAGE_ZOOM_OPTIONS: { value: ImageZoom; labelKey: string }[] = [
  { value: "fit", labelKey: "studio.raster.zoomFit" },
  { value: 1, labelKey: "studio.raster.zoom1to1" },
  { value: 2, labelKey: "studio.raster.zoom2x" },
  { value: 4, labelKey: "studio.raster.zoom4x" },
];

const imageStyle = computed(() => {
  if (imageZoom.value === "fit" || !imageDims.value) {
    return { maxWidth: "100%", maxHeight: "100%" };
  }
  return {
    width: `${imageDims.value.width * imageZoom.value}px`,
    maxWidth: "none",
    maxHeight: "none",
  };
});

function resetWorkbench() {
  tree.value = null;
  loading.value = false;
  fatalCode.value = null;
  expanded.value = new Set();
  autoExpanded.value = false;
  manifestCreated.value = null;
  clearViewer();
}

onMounted(() => {
  void loadTree();
});

// sheet 复用：切换项目时整体重置并重新加载。
watch(
  () => props.project.relPath,
  () => {
    resetWorkbench();
    void loadTree();
  },
);
</script>

<template>
  <section class="code-page">
    <header class="page-heading">
      <div>
        <p class="eyebrow">{{ $t("studio.code.sheetTitle") }}</p>
        <FTypography :header="1" spacing="none">{{ project.name }}</FTypography>
        <p v-if="tree" class="root-path">{{ tree.rootPath }}</p>
      </div>
      <div class="heading-actions">
        <span v-if="tree" class="tree-stats">
          {{
            $t("studio.code.stats", {
              files: tree.fileCount,
              folders: tree.folderCount,
            })
          }}
        </span>
        <button
          class="refresh-button"
          type="button"
          :disabled="loading"
          @click="loadTree(true)"
        >
          <FIcon :name="loading ? 'Loader2' : 'RefreshCw'" :size="13" aria-label="" />
          {{ $t("studio.code.refresh") }}
        </button>
        <button
          class="refresh-button close"
          type="button"
          :aria-label="$t('common.close')"
          :title="$t('common.close')"
          @click="emit('close')"
        >
          <FIcon name="X" :size="14" aria-label="" />
        </button>
      </div>
    </header>

    <p class="code-hint">{{ $t("studio.code.hint") }}</p>

    <p v-if="!isTauri()" class="web-hint">{{ $t("studio.raster.webHint") }}</p>

    <div v-else-if="fatalMessage" class="empty-state">
      <FIcon name="FolderOpen" :size="20" aria-label="" />
      <p>{{ fatalMessage }}</p>
    </div>

    <Resizable
      v-else
      direction="horizontal"
      auto-save-id="openscp:code-workbench:v1"
      class="code-shell"
    >
      <!-- 左：项目目录树 -->
      <ResizablePanel id="code-tree" :default-size="26" :min-size="16">
        <div class="tree-panel">
          <div v-if="tree?.truncated" class="tree-warning">
            {{ $t("studio.code.truncated") }}
          </div>
          <p v-if="!rows.length && !loading" class="tree-empty">
            {{ $t("studio.code.empty") }}
          </p>
          <button
            v-for="row in rows"
            :key="row.relativePath"
            type="button"
            class="tree-row"
            :class="{
              selected: selected?.relativePath === row.relativePath,
              folder: row.kind === 'folder',
            }"
            :style="{ paddingLeft: `${10 + row.depth * 14}px` }"
            @click="onRowClick(row)"
          >
            <FIcon :name="rowIcon(row)" :size="13" aria-label="" />
            <span class="tree-name" :title="row.name">{{ row.name }}</span>
            <span v-if="row.kind === 'file' && row.size !== null" class="tree-size">
              {{ formatCodeSize(row.size) }}
            </span>
          </button>
        </div>
      </ResizablePanel>

      <ResizableHandle
        orientation="horizontal"
        :label="$t('studio.code.resizeHint')"
      />

      <!-- 右：按扩展名分流的工作台查看器 -->
      <ResizablePanel id="code-viewer" :default-size="74" :min-size="40">
        <div class="viewer" :class="{ empty: !selected }">
          <template v-if="selected">
            <!-- 右上角 toolbar：路径 + 缩放（图片）/ 笔记 / more 浮层 -->
            <header class="viewer-head">
              <FIcon
                :name="rowIcon({ ...selected, depth: 0 })"
                :size="14"
                aria-label=""
              />
              <span class="viewer-path">{{ selected.relativePath }}</span>
              <span
                v-if="selected.size !== null"
                class="viewer-size"
              >{{ formatCodeSize(selected.size) }}</span>

              <!-- 图片缩放档位 -->
              <div
                v-if="viewerKind === 'image'"
                class="viewer-toolbar"
                role="group"
                :aria-label="$t('studio.code.imageZoomLabel')"
              >
                <button
                  v-for="option in IMAGE_ZOOM_OPTIONS"
                  :key="option.labelKey"
                  type="button"
                  class="tool-chip"
                  :class="{ active: imageZoom === option.value }"
                  @click="imageZoom = option.value"
                >
                  {{ $t(option.labelKey) }}
                </button>
              </div>

              <div class="viewer-toolbar">
                <button
                  class="tool-chip"
                  type="button"
                  @click="notesOpen = true"
                >
                  <FIcon name="StickyNote" :size="12" aria-label="" />
                  {{ $t("studio.code.notesButton") }}
                </button>
                <FPopover v-model:open="moreOpen" :width="300">
                  <template #trigger>
                    <button
                      class="tool-chip"
                      type="button"
                      :aria-label="$t('studio.code.more')"
                    >
                      <FIcon name="Ellipsis" :size="14" aria-label="" />
                    </button>
                  </template>
                  <div class="more-panel">
                    <h4>{{ $t("studio.code.more") }}</h4>
                    <dl class="more-meta">
                      <div>
                        <dt>{{ $t("studio.code.morePath") }}</dt>
                        <dd class="mono">{{ selected.relativePath }}</dd>
                      </div>
                      <div v-if="selected.size !== null">
                        <dt>{{ $t("studio.code.packageSize") }}</dt>
                        <dd>{{ formatCodeSize(selected.size) }}</dd>
                      </div>
                    </dl>
                    <!-- DBPF 统计（package 专用） -->
                    <template v-if="viewerKind === 'package' && packageInfo">
                      <dl class="more-meta">
                        <div>
                          <dt>{{ $t("studio.code.packageEntries") }}</dt>
                          <dd>{{ packageInfo.entryCount }}</dd>
                        </div>
                        <div>
                          <dt>{{ $t("studio.code.packageDecompressed") }}</dt>
                          <dd>{{ formatCodeSize(packageInfo.decompressedTotal) }}</dd>
                        </div>
                      </dl>
                      <h4>{{ $t("studio.code.packageTypes") }}</h4>
                      <ul class="package-types">
                        <li
                          v-for="entry in packageInfo.types"
                          :key="entry.typeId"
                        >
                          <span class="mono">0x{{ entry.typeId.toString(16).toUpperCase().padStart(8, "0") }}</span>
                          <span>{{ entry.count }}</span>
                        </li>
                      </ul>
                      <p class="more-hint">{{ $t("studio.code.packageOpenHint") }}</p>
                    </template>
                  </div>
                </FPopover>
              </div>
            </header>

            <p v-if="viewerLoading" class="viewer-state">
              {{ $t("common.loading") }}
            </p>
            <p v-else-if="viewerError" class="viewer-state error" role="alert">
              {{ viewerError }}
            </p>

            <!-- 文本族：FCode 高亮（shiki 语法映射，txt 纯文本） -->
            <div
              v-else-if="viewerKind === 'text' && textDoc"
              class="text-viewer"
            >
              <FCode :code="textContent" :lang="textLanguage" />
            </div>

            <!-- 图片：棋盘底 + 缩放 -->
            <div
              v-else-if="viewerKind === 'image' && imageSrc"
              class="image-viewer"
              :style="{ backgroundImage: `url(${checkerboardUrl})` }"
            >
              <img
                :src="imageSrc"
                :alt="$t('studio.code.imageAlt')"
                :style="imageStyle"
              />
            </div>

            <!-- DBPF 容器：资源列表 | 资源预览 双 Resizable 区 -->
            <Resizable
              v-else-if="viewerKind === 'package'"
              direction="horizontal"
              auto-save-id="openscp:code-package:v1"
              class="package-split"
            >
              <ResizablePanel id="code-package-entries" :default-size="42" :min-size="20">
                <div class="entries-panel">
                  <div class="pane-title">
                    {{ $t("studio.code.entriesTitle") }}
                    <span class="pane-count">{{ packageEntries.length }}</span>
                  </div>
                  <!-- 分类标签（类型直方图）+ TGI 搜索 -->
                  <div
                    v-if="packageInfo?.types.length"
                    class="entry-filters"
                  >
                    <div class="type-chips" role="group">
                      <button
                        type="button"
                        class="tool-chip"
                        :class="{ active: entryTypeFilter === null }"
                        @click="entryTypeFilter = null"
                      >
                        {{ $t("studio.code.entriesAll") }}
                      </button>
                      <button
                        v-for="type in packageInfo.types"
                        :key="type.typeId"
                        type="button"
                        class="tool-chip"
                        :class="{ active: entryTypeFilter === type.typeId }"
                        :title="`0x${type.typeId.toString(16).toUpperCase().padStart(8, '0')}`"
                        @click="entryTypeFilter = type.typeId"
                      >
                        {{ type.name }}
                        <span class="chip-count">{{ type.count }}</span>
                      </button>
                    </div>
                    <input
                      v-model="entrySearch"
                      class="entry-search"
                      type="text"
                      :placeholder="$t('studio.code.entriesSearch')"
                    />
                  </div>
                  <p v-if="entriesLoading" class="pane-state">
                    {{ $t("common.loading") }}
                  </p>
                  <p v-else-if="!filteredEntries.length" class="pane-state">
                    {{ $t("studio.code.entriesEmpty") }}
                  </p>
                  <button
                    v-for="entry in filteredEntries"
                    :key="`${entry.typeId}:${entry.groupId}:${entry.instanceId}`"
                    type="button"
                    class="entry-row"
                    :class="{
                      selected:
                        selectedEntry?.typeId === entry.typeId &&
                        selectedEntry?.instanceId === entry.instanceId,
                    }"
                    @click="selectEntry(entry)"
                  >
                    <span class="mono">0x{{ entry.typeId.toString(16).toUpperCase().padStart(8, "0") }}</span>
                    <span class="mono entry-instance">0x{{ entry.instanceId.toString(16).toUpperCase().padStart(8, "0") }}</span>
                    <span
                      v-if="entry.semantic"
                      class="semantic-tag"
                      :title="entry.semantic.id"
                    >{{ semanticLabel(entry) }}</span>
                    <span class="entry-size">{{ formatCodeSize(entry.decompressedSize) }}</span>
                  </button>
                </div>
              </ResizablePanel>
              <ResizableHandle
                orientation="horizontal"
                :label="$t('studio.code.resizeHint')"
              />
              <ResizablePanel id="code-package-resource" :default-size="58" :min-size="30">
                <div class="resource-panel">
                  <template v-if="selectedEntry">
                    <div class="pane-title mono">
                      0x{{ selectedEntry.typeId.toString(16).toUpperCase().padStart(8, "0") }}
                      : 0x{{ selectedEntry.groupId.toString(16).toUpperCase().padStart(8, "0") }}
                      : 0x{{ selectedEntry.instanceId.toString(16).toUpperCase().padStart(8, "0") }}
                    </div>
                    <p v-if="resourceLoading" class="pane-state">
                      {{ $t("common.loading") }}
                    </p>
                    <template v-else-if="resourcePreview">
                      <p v-if="resourcePreview.truncated" class="pane-state warn">
                        {{ $t("studio.code.previewTruncated") }}
                      </p>
                      <!-- property：条目表 -->
                      <div
                        v-if="resourcePreview.kind === 'property' && resourcePreview.entries"
                        class="property-table"
                      >
                        <div
                          v-for="entry in resourcePreview.entries"
                          :key="entry.hash"
                          class="property-row"
                        >
                          <span class="mono">0x{{ entry.hash.toString(16).toUpperCase().padStart(8, "0") }}</span>
                          <span class="property-type">{{ entry.typeName }}</span>
                          <span class="property-value" :title="entry.value">{{ entry.value }}</span>
                        </div>
                      </div>
                      <!-- 文本：FCode -->
                      <FCode
                        v-else-if="resourcePreview.kind === 'text' && resourcePreview.content !== null"
                        :code="resourcePreview.content"
                        lang="text"
                      />
                      <!-- hex dump -->
                      <pre v-else-if="resourcePreview.hexDump" class="hex-view">{{ resourcePreview.hexDump }}</pre>
                    </template>
                    <p v-else class="pane-state">
                      {{ $t("studio.code.resourceEmpty") }}
                    </p>
                  </template>
                  <p v-else class="pane-state">
                    {{ $t("studio.code.resourceEmpty") }}
                  </p>
                </div>
              </ResizablePanel>
            </Resizable>

            <!-- 其余二进制：占位 -->
            <p v-else class="viewer-state">{{ $t("studio.code.binaryHint") }}</p>
          </template>

          <template v-else>
            <FIcon name="ListTree" :size="20" aria-label="" />
            <p>{{ $t("studio.code.pickHint") }}</p>
          </template>
        </div>
      </ResizablePanel>
    </Resizable>

    <!-- 模组笔记（文档工作区 /mods/<模组名>/） -->
    <NotesSheet v-model:open="notesOpen" :project="project" />
  </section>
</template>

<style scoped>
.code-page {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding: 16px 18px;
}
.page-heading {
  align-items: flex-start;
  display: flex;
  justify-content: space-between;
}
.page-heading :deep(h1) {
  margin: 0;
}
.eyebrow {
  color: var(--primary);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.08em;
  margin: 0 0 8px;
}
.root-path {
  color: var(--subtle-foreground);
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  margin: 6px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.heading-actions {
  align-items: center;
  display: flex;
  gap: 10px;
}
.tree-stats {
  color: var(--muted-foreground);
  font-size: 11.5px;
  font-variant-numeric: tabular-nums;
}
.refresh-button {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11.5px;
  gap: 6px;
  min-height: 26px;
  padding: 0 10px;
}
.refresh-button:hover {
  color: var(--foreground);
}
.refresh-button:disabled {
  cursor: default;
  opacity: 0.6;
}
.refresh-button.close {
  padding: 0 7px;
}
.refresh-button.close:hover {
  background: color-mix(in srgb, var(--danger) 12%, transparent);
  color: var(--danger);
}
.code-hint {
  color: var(--subtle-foreground);
  font-size: 11.5px;
  margin: 0;
}
.web-hint {
  color: var(--warning);
  font-size: 12px;
}
.empty-state {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
}
.empty-state p {
  font-size: 12.5px;
  margin: 0;
}
.code-shell {
  flex: 1;
  min-height: 0;
}
.tree-panel {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  height: 100%;
  overflow: auto;
  padding: 6px;
}
.tree-warning {
  background: color-mix(in srgb, var(--warning) 12%, transparent);
  border-radius: var(--radius-sm);
  color: var(--warning);
  font-size: 11px;
  margin: 2px 2px 6px;
  padding: 4px 8px;
}
.tree-empty {
  color: var(--subtle-foreground);
  font-size: 11.5px;
  padding: 8px;
}
.tree-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 6px;
  min-height: 26px;
  padding-right: 8px;
  text-align: start;
  width: 100%;
}
.tree-row:hover {
  background: var(--surface-hover);
}
.tree-row.folder {
  color: var(--foreground);
  font-weight: 500;
}
.tree-row.selected {
  /* 选中态两套主题都必须可读：前景色 + 加粗，背景仅作位置提示。 */
  background: var(--surface-hover);
  color: var(--foreground);
  font-weight: 650;
}
.tree-row.selected .tree-size,
.tree-row.selected .tree-name {
  color: var(--foreground);
}
.tree-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tree-size {
  color: var(--subtle-foreground);
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
}
.viewer {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  padding: 12px;
}
.viewer.empty {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
}
.viewer.empty p {
  font-size: 12px;
  margin: 0;
}
.viewer-head {
  align-items: center;
  border-bottom: 1px solid var(--border);
  color: var(--muted-foreground);
  display: flex;
  gap: 8px;
  padding-bottom: 8px;
}
.viewer-path {
  flex: 1;
  font-family: var(--font-mono, monospace);
  font-size: 11.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.viewer-size {
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
}
.viewer-toolbar {
  align-items: center;
  display: flex;
  gap: 4px;
}
.tool-chip {
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
  min-height: 24px;
  padding: 0 8px;
}
.tool-chip:hover {
  color: var(--foreground);
}
.tool-chip.active {
  background: var(--surface-hover);
  color: var(--foreground);
}
.more-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.more-panel h4 {
  color: var(--subtle-foreground);
  font-size: 10.5px;
  letter-spacing: 0.06em;
  margin: 0;
  text-transform: uppercase;
}
.more-meta {
  display: grid;
  gap: 6px;
  margin: 0;
}
.more-meta dt {
  color: var(--subtle-foreground);
  font-size: 10.5px;
}
.more-meta dd {
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  margin: 1px 0 0;
  overflow-wrap: anywhere;
}
.more-hint {
  color: var(--subtle-foreground);
  font-size: 10.5px;
  margin: 0;
}
.mono {
  font-family: var(--font-mono, monospace);
}
.viewer-state {
  color: var(--muted-foreground);
  font-size: 12px;
}
.viewer-state.error {
  color: var(--warning);
}
.text-viewer {
  flex: 1;
  margin-top: 10px;
  min-height: 0;
  overflow: auto;
}
.image-viewer {
  align-items: center;
  background-color: var(--surface-elevated);
  background-size: 16px 16px;
  display: flex;
  flex: 1;
  justify-content: center;
  margin-top: 10px;
  min-height: 0;
  overflow: auto;
  padding: 12px;
}
.image-viewer img {
  image-rendering: pixelated;
}
.package-split {
  flex: 1;
  margin-top: 10px;
  min-height: 0;
}
.entries-panel,
.resource-panel {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: auto;
  padding: 8px;
}
.entry-filters {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-bottom: 8px;
}
.type-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.chip-count {
  color: var(--subtle-foreground);
  font-variant-numeric: tabular-nums;
}
.entry-search {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 11.5px;
  min-height: 26px;
  padding: 0 8px;
  width: 100%;
}
.entry-search:focus {
  outline: 1px solid var(--primary);
}
.semantic-tag {
  background: color-mix(in srgb, var(--primary) 14%, transparent);
  border-radius: 999px;
  color: var(--primary);
  flex: none;
  font-size: 10px;
  padding: 0 7px;
  white-space: nowrap;
}
.pane-title {
  align-items: center;
  color: var(--subtle-foreground);
  display: flex;
  font-size: 10.5px;
  font-weight: 700;
  gap: 6px;
  letter-spacing: 0.06em;
  padding: 2px 4px 8px;
  text-transform: uppercase;
}
.pane-count {
  background: var(--surface);
  border-radius: 999px;
  font-variant-numeric: tabular-nums;
  padding: 0 7px;
}
.pane-state {
  color: var(--subtle-foreground);
  font-size: 11.5px;
  padding: 8px 4px;
}
.pane-state.warn {
  color: var(--warning);
}
.entry-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  font-size: 11px;
  gap: 8px;
  min-height: 25px;
  padding: 0 6px;
  text-align: start;
  width: 100%;
}
.entry-row:hover {
  background: var(--surface-hover);
}
.entry-row.selected {
  background: var(--surface-hover);
  color: var(--foreground);
  font-weight: 650;
}
.entry-instance {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
}
.entry-size {
  color: var(--subtle-foreground);
  font-variant-numeric: tabular-nums;
}
.property-table {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.property-row {
  align-items: baseline;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  display: flex;
  font-size: 11.5px;
  gap: 10px;
  padding: 3px 4px;
}
.property-type {
  color: var(--subtle-foreground);
  flex: none;
  font-size: 10.5px;
  width: 72px;
}
.property-value {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.hex-view {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  line-height: 1.6;
  margin: 0;
  white-space: pre;
}
</style>
