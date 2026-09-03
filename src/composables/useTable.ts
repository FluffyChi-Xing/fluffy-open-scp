import { computed, onMounted, readonly, shallowRef } from 'vue'
import { useLoading } from '@/composables/useLoading'

export interface TableColumn<Row> {
  key: keyof Row | string
  titleKey: string
  sortable?: boolean
}

export interface TablePagination {
  page: number
  pageSize: number
}

export interface TableResult<Row> {
  rows: Row[]
  total: number
}

export type TableSource = 'local' | 'remote'

export interface UseTableOptions<Row, Query extends Record<string, unknown>, Result> {
  columns: TableColumn<Row>[]
  data?: Row[]
  request?: (query: Query & TablePagination) => Promise<Result>
  transform?: (result: Result) => TableResult<Row>
  initialQuery?: Query
  initialPageSize?: number
  initialSource?: TableSource
  immediate?: boolean
}

export function useTable<Row, Query extends Record<string, unknown> = Record<string, never>, Result = TableResult<Row>>(
  options: UseTableOptions<Row, Query, Result>
) {
  const sourceRows = shallowRef<Row[]>(options.data ?? [])
  const source = shallowRef<TableSource>(options.initialSource ?? (options.request ? 'remote' : 'local'))
  const rows = shallowRef<Row[]>([])
  const total = shallowRef(options.data?.length ?? 0)
  const error = shallowRef<unknown>()
  const query = shallowRef<Query>({ ...(options.initialQuery ?? {}) } as Query)
  const pagination = shallowRef<TablePagination>({ page: 1, pageSize: options.initialPageSize ?? 10 })
  const { loading, run } = useLoading()
  const pageCount = computed(() => Math.max(1, Math.ceil(total.value / pagination.value.pageSize)))

  function updateLocalRows() {
    total.value = sourceRows.value.length
    const start = (pagination.value.page - 1) * pagination.value.pageSize
    rows.value = sourceRows.value.slice(start, start + pagination.value.pageSize)
  }

  async function fetch() {
    error.value = undefined
    if (source.value === 'local' || !options.request) {
      updateLocalRows()
      return rows.value
    }

    const request = options.request
    return run(async () => {
      try {
        const result = await request({ ...query.value, ...pagination.value })
        const normalized = options.transform ? options.transform(result) : result as unknown as TableResult<Row>
        rows.value = normalized.rows
        total.value = normalized.total
        return rows.value
      } catch (nextError) {
        rows.value = []
        total.value = 0
        error.value = nextError
        throw nextError
      }
    })
  }

  async function reload() {
    pagination.value = { ...pagination.value, page: 1 }
    return fetch()
  }

  async function setSource(nextSource: TableSource) {
    source.value = nextSource
    return reload()
  }

  async function setPage(page: number) {
    pagination.value = { ...pagination.value, page: Math.min(Math.max(1, page), pageCount.value) }
    return fetch()
  }

  async function setPageSize(pageSize: number) {
    pagination.value = { page: 1, pageSize }
    return fetch()
  }

  async function reset() {
    query.value = { ...(options.initialQuery ?? {}) } as Query
    pagination.value = { page: 1, pageSize: options.initialPageSize ?? 10 }
    return fetch()
  }

  if (options.immediate !== false) onMounted(() => { void fetch() })

  return {
    columns: readonly(options.columns),
    source: readonly(source),
    rows: readonly(rows),
    total: readonly(total),
    loading,
    error: readonly(error),
    query,
    pagination: readonly(pagination),
    pageCount,
    fetch,
    reload,
    reset,
    setSource,
    setPage,
    setPageSize
  }
}
