import PackageExplorerPage from '@/pages/packages/index.vue'
import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'packages', path: 'packages', component: PackageExplorerPage, meta: { titleKey: 'navigation.packages', icon: 'deployment', groupKey: 'navigation.resources', order: 10 } }
  ]
} satisfies RouteModule
