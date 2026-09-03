<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { NavigationGroup, NavigationItem } from '@/router/types'
import FIcon from '@/components/extensions/FIcon.vue'

interface Props { groups: NavigationGroup[] }
interface Emits { navigate: []; openExternal: [item: NavigationItem] }

const props = defineProps<Props>()
const emit = defineEmits<Emits>()
const route = useRoute()
const router = useRouter()
const activeKey = computed(() => String(route.meta.activeMenu ?? route.name ?? ''))

function selectItem(item: NavigationItem) {
  if (item.external) {
    emit('openExternal', item)
    return
  }
  if (item.routeName) router.push({ name: item.routeName })
  emit('navigate')
}
</script>

<template>
  <nav class="sidebar-nav">
    <section v-for="group in props.groups" :key="group.key" class="nav-group">
      <p class="nav-label">{{ $t(group.titleKey) }}</p>
      <button v-for="item in group.items" :key="item.key" class="sidebar-link" :class="{ active: item.key === activeKey }" type="button" @click="selectItem(item)">
        <FIcon :name="item.icon ?? 'Menu'" size="16" />
        <span>{{ $t(item.titleKey) }}</span>
      </button>
    </section>
  </nav>
</template>

<style scoped>
.sidebar-nav{display:flex;flex:1;flex-direction:column;gap:18px;padding-top:15px}.nav-group{display:grid;gap:3px}.nav-label{color:var(--subtle-foreground);display:block;font-size:10px;font-weight:700;letter-spacing:.08em;margin:0 0 7px;padding-inline:8px;text-transform:uppercase}.sidebar-link{align-items:center;background:transparent;border:0;border-radius:var(--radius-sm);color:var(--muted-foreground);cursor:pointer;display:flex;font-size:13px;gap:10px;padding:8px;text-align:start;transition:background-color 140ms ease,color 140ms ease,scale 140ms ease;width:100%}.sidebar-link:hover{background:var(--sidebar-hover);color:var(--foreground)}.sidebar-link:active{scale:.98}.sidebar-link.active{background:var(--sidebar-active);color:var(--sidebar-active-foreground);font-weight:650}.sidebar-link :deep(svg){flex:none}
</style>
