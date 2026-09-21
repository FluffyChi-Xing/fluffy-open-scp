<script setup lang="ts">
import { computed, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
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
  type CodeRow,
  type CodeTreeNodeDto,
} from "./code-support";
import type {
  CodeTextDocument,
  CodeTreeResponse,
  CodePackageInfo,
  ModProjectView,
} from "@/api/tauri";
import { rgbaBase64ToPngDataUrl, makeCheckerboard } from "@/lib/raster-editor/encoders";

/**
 * Code 工作台（M-CM1 文件浏览 + M-CM2 解析预览）：
 * 开发工作台模组列表「开发」按钮展开的全屏 sheet 内容，root = 该项目的
 * 文件夹（modRoot/<relPath>，后端同套越界/符号链接防护）。
 * 左侧 = 项目目录树（code_tree，目录优先、截断保护），
 * 右侧 = 按扩展名分流的工作台查看器——文本族走 code_read_text 行号视图
 * （JSON 美化）、图片走 read_image_rgba 棋盘预览、.package 走
 * code_package_info 只读统计（独立打开，不进全局包管理器）。
 * 编辑保存（code_write_text）与打包导出属 M-CM3，此处暂只读。
 */
const props = defineProps<{ project: ModProjectView }>();

const { t } = useI18n();
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
const packageInfo = shallowRef<CodePackageInfo | null>(null);
const viewerError = ref("");

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
  packageInfo.value = null;
  viewerError.value = "";
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
  packageInfo.value = null;
  viewerError.value = "";
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
      imageSrc.value = rgbaBase64ToPngDataUrl(
        image.rgbaBase64,
        image.width,
        image.height,
      );
    } else if (kind === "package") {
      packageInfo.value = await tauriApi.workspace.codePackageInfo(
        props.project.relPath,
        node.relativePath,
      );
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

const viewerKind = computed(() =>
  selected.value ? classifyCodeFile(selected.value.name) : null,
);

const checkerboardUrl = makeCheckerboard();

const isJson = computed(
  () => extensionOf(selected.value?.name ?? "") === "json",
);

const textLines = computed(() =>
  textDoc.value ? textDoc.value.content.split("\n").length : 0,
);

const gutterText = computed(() =>
  Array.from({ length: textLines.value }, (_, index) => index + 1).join("\n"),
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

function resetWorkbench() {
  tree.value = null;
  loading.value = false;
  fatalCode.value = null;
  expanded.value = new Set();
  autoExpanded.value = false;
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
        <p v-if="tree" class="code-hint">{{ tree.rootPath }}</p>
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
      <!-- 左：modRoot 目录树 -->
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
            </header>

            <p v-if="viewerLoading" class="viewer-state">
              {{ $t("common.loading") }}
            </p>
            <p v-else-if="viewerError" class="viewer-state error" role="alert">
              {{ viewerError }}
            </p>

            <!-- 文本族：行号视图（JSON 美化） -->
            <div
              v-else-if="viewerKind === 'text' && textDoc"
              class="text-viewer"
            >
              <pre class="text-gutter" aria-hidden="true">{{ gutterText }}</pre>
              <pre class="text-content">{{ textContent }}</pre>
            </div>

            <!-- 图片：棋盘底预览 -->
            <div
              v-else-if="viewerKind === 'image' && imageSrc"
              class="image-viewer"
              :style="{ backgroundImage: `url(${checkerboardUrl})` }"
            >
              <img :src="imageSrc" :alt="$t('studio.code.imageAlt')" />
            </div>

            <!-- DBPF 容器：只读统计 -->
            <div
              v-else-if="viewerKind === 'package' && packageInfo"
              class="package-viewer"
            >
              <h3>{{ $t("studio.code.packageCard") }}</h3>
              <dl class="package-meta">
                <div>
                  <dt>{{ $t("studio.code.packageEntries") }}</dt>
                  <dd>{{ packageInfo.entryCount }}</dd>
                </div>
                <div>
                  <dt>{{ $t("studio.code.packageDecompressed") }}</dt>
                  <dd>{{ formatCodeSize(packageInfo.decompressedTotal) }}</dd>
                </div>
                <div>
                  <dt>{{ $t("studio.code.packageSize") }}</dt>
                  <dd>{{ formatCodeSize(packageInfo.size) }}</dd>
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
              <p class="package-hint">{{ $t("studio.code.packageOpenHint") }}</p>
            </div>

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
  </section>
</template>

<style scoped>
.code-page {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding: 18px 22px 16px;
}
.page-heading {
  align-items: flex-start;
  display: flex;
  justify-content: space-between;
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
  background: var(--surface-hover);
  color: var(--accent);
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
.viewer-state {
  color: var(--muted-foreground);
  font-size: 12px;
}
.viewer-state.error {
  color: var(--warning);
}
.text-viewer {
  display: flex;
  flex: 1;
  gap: 0;
  margin-top: 10px;
  min-height: 0;
  overflow: auto;
}
.text-gutter {
  color: var(--subtle-foreground);
  font-family: var(--font-mono, monospace);
  font-size: 11.5px;
  line-height: 1.55;
  margin: 0;
  padding: 8px 10px 8px 0;
  text-align: end;
  user-select: none;
}
.text-content {
  color: var(--foreground);
  font-family: var(--font-mono, monospace);
  font-size: 11.5px;
  line-height: 1.55;
  margin: 0;
  padding: 8px;
  white-space: pre;
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
  max-height: 100%;
  max-width: 100%;
  object-fit: contain;
}
.package-viewer {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 10px;
  overflow: auto;
}
.package-viewer h3 {
  font-size: 13px;
  margin: 0;
}
.package-viewer h4 {
  color: var(--muted-foreground);
  font-size: 11.5px;
  margin: 6px 0 0;
}
.package-meta {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
}
.package-meta dt {
  color: var(--subtle-foreground);
  font-size: 10.5px;
}
.package-meta dd {
  font-size: 16px;
  font-variant-numeric: tabular-nums;
  margin: 2px 0 0;
}
.package-types {
  display: flex;
  flex-direction: column;
  gap: 2px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.package-types li {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  font-variant-numeric: tabular-nums;
  font-size: 11.5px;
  justify-content: space-between;
  min-height: 24px;
  padding: 0 8px;
}
.package-types .mono {
  font-family: var(--font-mono, monospace);
}
.package-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
}
</style>
