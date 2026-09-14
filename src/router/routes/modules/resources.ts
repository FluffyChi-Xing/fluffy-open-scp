import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'packages', path: 'packages', component: () => import('@/pages/packages/index.vue'), meta: { titleKey: 'navigation.packages', icon: 'deployment', groupKey: 'navigation.resources', order: 10 } }
  ]
} satisfies RouteModule
