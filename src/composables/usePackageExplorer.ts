import { computed, shallowRef } from 'vue'
import { createDataSource } from '@/api/data-source'
import { type OpenPackageResponse, type ResourceBytes, type ResourcePage, type ResourceSummary, type Tgi } from '@/api'

export function usePackageExplorer() {
  const opened = shallowRef<OpenPackageResponse | null>(null)
  const page = shallowRef<ResourcePage | null>(null)
  const selected = shallowRef<ResourceSummary | null>(null)
  const bytes = shallowRef<ResourceBytes | null>(null)
  const loading = shallowRef(false)
  const error = shallowRef('')
  const filter = shallowRef('')
  const offset = shallowRef(0)
  const limit = shallowRef(100)
  const hasPrevious = computed(() => offset.value > 0)
  const hasNext = computed(() => Boolean(page.value && offset.value + page.value.items.length < page.value.total))
  const source = createDataSource()

  async function open(path: string) {
    loading.value = true
    error.value = ''
    try {
      opened.value = await source.openPackage(path)
      page.value = opened.value.resources
      offset.value = 0
      selected.value = null
      bytes.value = null
    } catch (cause) { error.value = messageOf(cause) } finally { loading.value = false }
  }
  async function loadPage(nextOffset = offset.value) {
    if (!opened.value) return
    loading.value = true
    error.value = ''
    try {
      page.value = await source.listResources(opened.value.package.packageId, nextOffset, limit.value, filter.value || undefined)
      offset.value = nextOffset
    } catch (cause) { error.value = messageOf(cause) } finally { loading.value = false }
  }
  async function applyFilter() { await loadPage(0) }
  async function selectResource(resource: ResourceSummary) {
    selected.value = resource
    bytes.value = null
    error.value = ''
    try { bytes.value = await source.readResourceBytes(opened.value!.package.packageId, resource.tgi, 0, Math.min(4096, resource.decompressedSize)) } catch (cause) { error.value = messageOf(cause) }
  }
  async function close() {
    if (!opened.value) return
    await source.closePackage(opened.value.package.packageId)
    opened.value = null
    page.value = null
    selected.value = null
    bytes.value = null
  }
  function tgiLabel(tgi: Tgi) { return `${hex(tgi.typeId)}:${hex(tgi.group)}:${hex(tgi.instance)}` }
  return { opened, page, selected, bytes, loading, error, filter, offset, limit, hasPrevious, hasNext, open, loadPage, applyFilter, selectResource, close, tgiLabel }
}

function hex(value: number) { return value.toString(16).padStart(8, '0') }
function messageOf(cause: unknown) {
  if (cause && typeof cause === 'object' && 'message' in cause) return String(cause.message)
  return cause instanceof Error ? cause.message : String(cause)
}
