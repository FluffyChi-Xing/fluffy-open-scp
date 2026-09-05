import { computed, shallowRef } from 'vue'
import { tauriApi, type MarkdownDocument, type WorkspaceFolder, type WorkspaceStatus } from '@/api'

export function useWorkspace() {
  const status = shallowRef<WorkspaceStatus | null>(null)
  const folders = shallowRef<WorkspaceFolder[]>([])
  const selectedPath = shallowRef('')
  const document = shallowRef<MarkdownDocument | null>(null)
  const loading = shallowRef(false)
  const saving = shallowRef(false)
  const error = shallowRef('')
  const isConfigured = computed(() => status.value?.configured === true && status.value.available)

  async function loadStatus() {
    loading.value = true
    error.value = ''
    try { status.value = await tauriApi.workspace.get() } catch (cause) { error.value = messageOf(cause) } finally { loading.value = false }
  }
  async function loadFolders() {
    loading.value = true
    error.value = ''
    try { folders.value = await tauriApi.workspace.list() } catch (cause) { error.value = messageOf(cause) } finally { loading.value = false }
  }
  async function select(path: string) {
    selectedPath.value = path
    document.value = null
    error.value = ''
    try { document.value = await tauriApi.workspace.readMarkdown(path) } catch (cause) { error.value = messageOf(cause) }
  }
  async function setRoot(path: string) { status.value = await tauriApi.workspace.setRoot(path); folders.value = []; document.value = null; selectedPath.value = '' }
  async function createFolder(path: string) { folders.value = await tauriApi.workspace.createFolder(path) }
  async function save(content: string) {
    if (!document.value) return
    saving.value = true
    error.value = ''
    try {
      document.value = await tauriApi.workspace.writeMarkdown(document.value.relativePath, content, document.value.revision)
    } catch (cause) { error.value = messageOf(cause) } finally { saving.value = false }
  }
  return { status, folders, selectedPath, document, loading, saving, error, isConfigured, loadStatus, loadFolders, select, setRoot, createFolder, save }
}

function messageOf(cause: unknown) {
  if (cause && typeof cause === 'object' && 'message' in cause) return String(cause.message)
  return cause instanceof Error ? cause.message : String(cause)
}
