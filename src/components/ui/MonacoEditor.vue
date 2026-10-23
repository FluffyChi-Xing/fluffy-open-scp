<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import * as monaco from "monaco-editor";

const props = withDefaults(defineProps<{ value: string; language?: string; readOnly?: boolean }>(), {
  language: "plaintext",
  readOnly: true,
});
const host = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let model: monaco.editor.ITextModel | null = null;
let themeObserver: MutationObserver | undefined;

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
  if (language === "er2") return "er2";
  if (language === "text") return "plaintext";
  return language;
}

function registerEr2() {
  if (monaco.languages.getLanguages().some((item) => item.id === "er2")) return;
  monaco.languages.register({ id: "er2", extensions: [".er2"] });
  monaco.languages.setMonarchTokensProvider("er2", {
    tokenizer: {
      root: [
        [/\/\/.*$/, "comment"],
        [/[{}()[\],;]/, "delimiter"],
        [/[a-zA-Z_][\w-]*/, { cases: { "@keywords": "keyword", "@default": "identifier" } }],
        [/\d+(?:\.\d+)?/, "number"],
        [/"(?:[^"\\]|\\.)*"/, "string"],
      ],
    },
    keywords: ["unitRule", "globalRule", "define", "create", "set", "rate", "applyCount", "onSuccess", "onFail", "chain"],
  });
}

onMounted(async () => {
  await nextTick();
  if (!host.value) return;
  defineThemes();
  syncTheme();
  themeObserver = new MutationObserver(syncTheme);
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  registerEr2();
  model = monaco.editor.createModel(props.value, monacoLanguage(props.language));
  editor = monaco.editor.create(host.value, {
    model,
    // Source parsing is a preview surface. Keep it read-only even if a caller
    // accidentally passes a falsey value while reusing this component.
    readOnly: true,
    domReadOnly: true,
    contextmenu: false,
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
});

watch(() => props.value, (value) => {
  if (model && model.getValue() !== value) model.setValue(value);
});
watch(() => props.language, (value) => {
  if (model) monaco.editor.setModelLanguage(model, monacoLanguage(value));
});

onBeforeUnmount(() => {
  themeObserver?.disconnect();
  editor?.dispose();
  model?.dispose();
  editor = null;
  model = null;
});
</script>

<template><div ref="host" class="monaco-editor-host" /></template>

<style scoped>
.monaco-editor-host { min-height: 260px; height: min(65vh, 620px); width: 100%; overflow: hidden; border: 1px solid var(--border); border-radius: var(--radius-lg); }
</style>
