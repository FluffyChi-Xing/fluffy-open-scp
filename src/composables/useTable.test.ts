import { nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import { useTable } from '@/composables/useTable'

describe('useTable', () => {
  it('paginates local data after an explicit fetch', async () => {
    const table = useTable({ columns: [], data: [{ id: 1 }, { id: 2 }, { id: 3 }], initialPageSize: 2, immediate: false })
    await table.fetch()
    expect(table.rows.value).toEqual([{ id: 1 }, { id: 2 }])
    await table.setPage(2)
    expect(table.rows.value).toEqual([{ id: 3 }])
  })

  it('switches between local and requested data', async () => {
    const table = useTable({
      columns: [],
      data: [{ id: 1 }],
      request: async () => ({ rows: [{ id: 2 }], total: 1 }),
      initialSource: 'local',
      immediate: false
    })

    await table.fetch()
    expect(table.rows.value).toEqual([{ id: 1 }])
    await table.setSource('remote')
    expect(table.rows.value).toEqual([{ id: 2 }])
  })

  it('normalizes remote results with transform', async () => {
    const table = useTable<{ id: number }, Record<string, never>, { payload: { data: { id: number }[]; total: number } }>({
      columns: [],
      request: async () => ({ payload: { data: [{ id: 8 }], total: 9 } }),
      transform: (result) => ({ rows: result.payload.data, total: result.payload.total }),
      immediate: false
    })
    await table.fetch()
    await nextTick()
    expect(table.rows.value).toEqual([{ id: 8 }])
    expect(table.total.value).toBe(9)
  })
})
