<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import * as monaco from "monaco-editor";
import { registerEr2 } from "./er2-language";

const props = withDefaults(defineProps<{ value: string; language?: string; fill?: boolean; editable?: boolean }>(), {
  language: "plaintext",
  fill: false,
  editable: false,
});
const emit = defineEmits<{ "update:value": [string] }>();
const host = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let model: monaco.editor.ITextModel | null = null;
let themeObserver: MutationObserver | undefined;
/** 程序化 setValue 也会触发 onDidChangeModelContent——标记以免回环。 */
let applyingExternalValue = false;

function defineThemes() {
  monaco.editor.defineTheme("openscp-light", {
    base: "vs", inherit: true, rules: [],
    colors: { "editor.background": "#ffffff", "editorGutter.background": "#ffffff" },
  });
  monaco.editor.defineTheme("openscp-dark", {
    base: "vs-dark", inherit: true, rules: [],
    colors: { "editor.background": "#111827", "editorGutter.background": "#111827" },
  });
}

function syncTheme() {
  monaco.editor.setTheme(document.documentElement.dataset.theme === "dark" ? "openscp-dark" : "openscp-light");
}

function monacoLanguage(language: string): string {
  if (language === "text") return "plaintext";
  return language;
}

onMounted(async () => {
  await nextTick();
  if (!host.value) return;
  defineThemes();
  syncTheme();
  themeObserver = new MutationObserver(syncTheme);
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  registerEr2(monaco.languages);
  model = monaco.editor.createModel(props.value, monacoLanguage(props.language));
  editor = monaco.editor.create(host.value, {
    model,
    // Default remains a read-only preview surface; editing is an explicit
    // opt-in (`editable`) that also enables the `update:value` stream.
    readOnly: !props.editable,
    domReadOnly: !props.editable,
    contextmenu: props.editable,
    dragAndDrop: false,
    automaticLayout: true,
    minimap: { enabled: false },
    padding: { top: 12, bottom: 12 },
    scrollBeyondLastLine: false,
    fontSize: 12,
    lineNumbers: "on",
    renderWhitespace: "selection",
    wordWrap: "off",
  });
  editor.onDidChangeModelContent(() => {
    if (!applyingExternalValue) emit("update:value", model?.getValue() ?? "");
  });
});

watch(() => props.value, (value) => {
  if (model && model.getValue() !== value) {
    applyingExternalValue = true;
    model.setValue(value);
    applyingExternalValue = false;
  }
});
watch(() => props.language, (value) => {
  if (model) monaco.editor.setModelLanguage(model, monacoLanguage(value));
});
watch(() => props.editable, (editable) => {
  editor?.updateOptions({ readOnly: !editable, domReadOnly: !editable, contextmenu: editable });
});

onBeforeUnmount(() => {
  themeObserver?.disconnect();
  editor?.dispose();
  model?.dispose();
  editor = null;
  model = null;
});
</script>

<template><div ref="host" class="monaco-editor-host" :class="{ fill: props.fill }" /></template>

<style scoped>
.monaco-editor-host { min-height: 260px; height: min(65vh, 620px); width: 100%; overflow: hidden; border: 1px solid var(--border); border-radius: var(--radius-lg); }
/* 嵌入侧栏等固定高度容器：撑满宿主而非视口比例（automaticLayout 跟随） */
.monaco-editor-host.fill { height: 100%; min-height: 0; border: 0; border-radius: 0; }
</style>
