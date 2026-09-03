<script setup lang="ts">
import { computed, shallowRef } from 'vue'
import FButton from '@/components/ui/FButton.vue'
import FPanel from '@/components/ui/FPanel.vue'
import FSpinner from '@/components/ui/FSpinner.vue'
import { useTable } from '@/composables/useTable'

interface TableRow { name: string; owner: string; status: string; updated: string }
const query = shallowRef('')
const descending = shallowRef(false)
const sourceRows: TableRow[] = [
  { name: 'console-web', owner: 'Fluffy', status: 'Ready', updated: '4m' },
  { name: 'edge-api', owner: 'Mina', status: 'Building', updated: '18m' },
  { name: 'design-system', owner: 'Leo', status: 'Review', updated: '1d' },
  { name: 'docs-portal', owner: 'Aya', status: 'Ready', updated: '2d' },
  { name: 'billing-core', owner: 'Sora', status: 'Ready', updated: '3d' },
  { name: 'auth-gateway', owner: 'Nora', status: 'Building', updated: '4d' }
]
const request = async ({ page, pageSize }: { page: number; pageSize: number }) => {
  await new Promise((resolve) => window.setTimeout(resolve, 450))
  const start = (page - 1) * pageSize
  return { payload: { items: sourceRows.slice(start, start + pageSize), count: sourceRows.length } }
}
const table = useTable<TableRow, Record<string, never>, { payload: { items: TableRow[]; count: number } }>({
  columns: [{ key: 'name', titleKey: 'table.name', sortable: true }, { key: 'owner', titleKey: 'table.owner' }, { key: 'status', titleKey: 'table.status' }, { key: 'updated', titleKey: 'table.updated' }],
  data: sourceRows,
  request,
  transform: (result) => ({ rows: result.payload.items, total: result.payload.count }),
  initialPageSize: 3,
  initialSource: 'local'
})
const visibleRows = computed(() => [...table.rows.value.filter((row) => row.name.includes(query.value.toLowerCase()))].sort((a, b) => descending.value ? b.name.localeCompare(a.name) : a.name.localeCompare(b.name)))

async function changeSource() {
  await table.setSource(table.source.value === 'local' ? 'remote' : 'local')
}
</script>

<template><section class="page"><header class="page-header"><div><p class="eyebrow">{{ $t('showcase.eyebrow') }}</p><h1>{{ $t('showcase.tableTitle') }}</h1><p>{{ $t('showcase.tableDescription') }}</p></div><div class="header-actions"><input v-model="query" :placeholder="$t('showcase.filter')" /><FButton variant="secondary" :loading="table.loading.value" @click="changeSource">{{ table.source.value === 'local' ? $t('table.remote') : $t('table.refresh') }}</FButton></div></header><FPanel><div class="data-table"><div class="table-row table-heading"><button type="button" @click="descending=!descending">{{ $t('table.name') }} ↕</button><span>{{ $t('table.owner') }}</span><span>{{ $t('table.status') }}</span><span>{{ $t('table.updated') }}</span></div><div v-if="table.loading.value" class="table-loading"><FSpinner :label="$t('common.loading')" />{{ $t('common.loading') }}</div><div v-else-if="table.error.value" class="empty">{{ $t('table.loadError') }}</div><div v-for="row in visibleRows" :key="row.name" class="table-row"><strong>{{ row.name }}</strong><span>{{ row.owner }}</span><span class="status" :class="row.status.toLowerCase()">{{ row.status }}</span><time>{{ row.updated }}</time></div><p v-if="!table.loading.value && !table.error.value && !visibleRows.length" class="empty">{{ $t('showcase.noResults') }}</p></div><footer class="pagination"><span>{{ $t('table.total', { total: table.total.value }) }}</span><div><FButton variant="ghost" :disabled="table.pagination.value.page === 1" @click="table.setPage(table.pagination.value.page - 1)">{{ $t('table.previous') }}</FButton><span>{{ $t('table.page', { page: table.pagination.value.page }) }}</span><FButton variant="ghost" :disabled="table.pagination.value.page === table.pageCount.value" @click="table.setPage(table.pagination.value.page + 1)">{{ $t('table.next') }}</FButton></div></footer></FPanel></section></template>

<style scoped>
.page{display:grid;gap:24px}.page-header{align-items:flex-end;display:flex;gap:18px;justify-content:space-between}.eyebrow{color:var(--primary);font-size:11px;font-weight:750;letter-spacing:.08em;margin:0 0 10px;text-transform:uppercase}.page-header h1{font-size:clamp(1.8rem,3vw,2.5rem);letter-spacing:-.045em;margin:0}.page-header p:not(.eyebrow){color:var(--muted-foreground);font-size:14px;margin:10px 0 0}.header-actions{align-items:center;display:flex;gap:10px}.header-actions input{background:var(--surface);border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--foreground);font-size:13px;min-height:38px;padding:0 12px}.table-row{align-items:center;border-top:1px solid var(--border);display:grid;gap:14px;grid-template-columns:1.4fr 1fr .8fr .4fr;padding:14px 0}.table-heading{border:0;color:var(--subtle-foreground);font-size:10px;font-weight:700;letter-spacing:.06em;text-transform:uppercase}.table-heading button{background:transparent;border:0;color:inherit;cursor:pointer;font:inherit;padding:0;text-align:start}.table-row strong{font-size:13px}.table-row span,.table-row time{color:var(--muted-foreground);font-size:12px}.status{font-weight:700}.status.ready{color:var(--success)}.status.building{color:var(--warning)}.status.review{color:var(--primary)}.empty{color:var(--muted-foreground);font-size:13px;padding:28px;text-align:center}.table-loading{align-items:center;color:var(--muted-foreground);display:flex;font-size:13px;gap:9px;justify-content:center;padding:34px}.pagination{align-items:center;border-top:1px solid var(--border);color:var(--subtle-foreground);display:flex;font-size:12px;justify-content:space-between;padding-top:14px}.pagination>div{align-items:center;display:flex;gap:5px}@media(max-width:720px){.page-header{align-items:stretch;flex-direction:column}.header-actions{align-items:stretch;flex-direction:column}.table-row{grid-template-columns:1fr auto}.table-row :nth-child(2),.table-row :nth-child(4),.table-heading{display:none}.pagination{align-items:flex-start;gap:10px;flex-direction:column}}
</style>
