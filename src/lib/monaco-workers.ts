import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import JsonWorker from "monaco-editor/languages/features/json/json.worker.js?worker";

// Explicit Vite workers also work from Tauri's bundled custom protocol.
Object.assign(globalThis, {
  MonacoEnvironment: {
    getWorker(_moduleId: string, label: string) {
      return label === "json" ? new JsonWorker() : new EditorWorker();
    },
  },
});
