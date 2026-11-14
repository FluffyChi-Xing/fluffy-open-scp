<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import {
  continueList,
  indent,
  insertCodeBlock,
  insertLink,
  insertTable,
  outdent,
  toggleHeading,
  toggleLineMarker,
  unclosedFenceStartLine,
  wrapInline,
  type TextEdit,
  type TextState,
} from "../markdown-input";

/**
 * Markdown 编辑器：无边框等宽文本域撑满面板，工具栏与快捷键优化录入——
 * Enter 自动续列表/待办、Tab/Shift+Tab 缩进、Ctrl+S 保存、
 * Ctrl+B/I 加粗斜体。变换逻辑全部在 markdown-input.ts（纯函数）。
 * IME 组合输入（中文拼音）期间不拦截任何按键。
 */
const props = defineProps<{ draft: string }>();
const emit = defineEmits<{
  "update:draft": [value: string];
  save: [];
}>();
const { t } = useI18n();
const textarea = ref<HTMLTextAreaElement | null>(null);
let pendingSelection: { start: number; end: number } | null = null;

/** 未闭合围栏会让预览把其后内容全部当代码——实时提示行号（-1 = 无）。 */
const fenceWarningLine = computed(() => unclosedFenceStartLine(props.draft));

interface ToolButton {
  icon: string;
  title: string;
  run: () => void;
}

function bold() {
  apply(wrapInline(currentState(), "**", t("workspace.toolBoldText")));
}
function italic() {
  apply(wrapInline(currentState(), "*", t("workspace.toolItalicText")));
}

const tools = computed<ToolButton[]>(() => [
  {
    icon: "Heading1",
    title: t("workspace.toolHeading1"),
    run: () => apply(toggleHeading(currentState(), 1)),
  },
  {
    icon: "Heading2",
    title: t("workspace.toolHeading2"),
    run: () => apply(toggleHeading(currentState(), 2)),
  },
  { icon: "Bold", title: t("workspace.toolBold"), run: bold },
  { icon: "Italic", title: t("workspace.toolItalic"), run: italic },
  {
    icon: "CodeXml",
    title: t("workspace.toolInlineCode"),
    run: () =>
      apply(wrapInline(currentState(), "`", t("workspace.toolCodeText"))),
  },
  {
    icon: "Braces",
    title: t("workspace.toolCodeBlock"),
    run: () => apply(insertCodeBlock(currentState())),
  },
  {
    icon: "Quote",
    title: t("workspace.toolQuote"),
    run: () => apply(toggleLineMarker(currentState(), "quote")),
  },
  {
    icon: "List",
    title: t("workspace.toolBulletList"),
    run: () => apply(toggleLineMarker(currentState(), "bullet")),
  },
  {
    icon: "ListOrdered",
    title: t("workspace.toolOrderedList"),
    run: () => apply(toggleLineMarker(currentState(), "ordered")),
  },
  {
    icon: "ListTodo",
    title: t("workspace.toolTaskList"),
    run: () => apply(toggleLineMarker(currentState(), "task")),
  },
  {
    icon: "Link",
    title: t("workspace.toolLink"),
    run: () =>
      apply(
        insertLink(
          currentState(),
          t("workspace.toolLinkText"),
          t("workspace.toolLinkUrl"),
        ),
      ),
  },
  {
    icon: "Table2",
    title: t("workspace.toolTable"),
    run: () => {
      const head = t("workspace.toolTableHead");
      apply(insertTable(currentState(), [head, head, head]));
    },
  },
]);

function currentState(): TextState {
  const el = textarea.value;
  return {
    text: el ? el.value : props.draft,
    selectionStart: el ? el.selectionStart : props.draft.length,
    selectionEnd: el ? el.selectionEnd : props.draft.length,
  };
}

/** 应用纯函数变换：等 DOM 更新后恢复选区（textarea 值替换会重置光标）。 */
function apply(edit: TextEdit) {
  pendingSelection = { start: edit.selectionStart, end: edit.selectionEnd };
  emit("update:draft", edit.text);
  void nextTick(() => {
    const el = textarea.value;
    if (!el || !pendingSelection) return;
    el.focus();
    el.setSelectionRange(pendingSelection.start, pendingSelection.end);
    pendingSelection = null;
  });
}

function onInput(event: Event) {
  emit("update:draft", (event.target as HTMLTextAreaElement).value);
}

function onKeyDown(event: KeyboardEvent) {
  // IME 组合中（拼音/笔画）：keyCode 229 兼容旧实现，Enter 应上屏候选词
  if (event.isComposing || event.keyCode === 229) return;
  const mod = event.ctrlKey || event.metaKey;
  if (mod) {
    const key = event.key.toLowerCase();
    if (key === "s") {
      event.preventDefault();
      emit("save");
      return;
    }
    if (key === "b") {
      event.preventDefault();
      bold();
      return;
    }
    if (key === "i") {
      event.preventDefault();
      italic();
      return;
    }
    return;
  }
  if (event.key === "Tab") {
    event.preventDefault();
    apply(event.shiftKey ? outdent(currentState()) : indent(currentState()));
    return;
  }
  if (event.key === "Enter" && !event.shiftKey) {
    const edit = continueList(currentState());
    if (edit) {
      event.preventDefault();
      apply(edit);
    }
  }
}
</script>

<template>
  <div class="md-editor">
    <div
      class="editor-toolbar"
      role="toolbar"
      :aria-label="$t('workspace.toolbarLabel')"
    >
      <button
        v-for="tool in tools"
        :key="tool.icon"
        class="tool-button"
        type="button"
        :title="tool.title"
        :aria-label="tool.title"
        @click="tool.run"
      >
        <FIcon :name="tool.icon" :size="14" aria-label="" />
      </button>
      <span v-if="fenceWarningLine >= 0" class="fence-hint" role="status">
        <FIcon name="Info" :size="12" aria-label="" />
        {{ $t("workspace.fenceUnclosed", { line: fenceWarningLine + 1 }) }}
      </span>
    </div>
    <textarea
      ref="textarea"
      class="editor-area"
      :value="props.draft"
      :aria-label="$t('workspace.markdownLabel')"
      spellcheck="false"
      @input="onInput"
      @keydown="onKeyDown"
    />
  </div>
</template>

<style scoped>
.md-editor {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
}
.editor-toolbar {
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: none;
  flex-wrap: wrap;
  gap: 2px;
  padding: 4px 10px;
}
.tool-button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  height: 26px;
  justify-content: center;
  padding: 0;
  width: 26px;
}
.tool-button:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.tool-button:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 1px;
}
.fence-hint {
  align-items: center;
  align-self: center;
  color: var(--warning, #e2b341);
  display: inline-flex;
  font-size: 11px;
  gap: 5px;
  margin-inline-start: auto;
  white-space: nowrap;
}
.editor-area {
  background: transparent;
  border: 0;
  color: var(--foreground);
  display: block;
  flex: 1;
  font:
    13px/1.75 ui-monospace,
    SFMono-Regular,
    Menlo,
    Consolas,
    monospace;
  margin: 0 auto;
  min-height: 0;
  outline: none;
  overflow: auto;
  padding: 16px 18px 40px;
  resize: none;
  tab-size: 2;
  width: min(100%, 880px);
}
.editor-area:focus-visible {
  outline: none;
}
</style>
