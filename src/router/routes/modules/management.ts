import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'settings', path: 'settings', component: () => import('@/pages/settings/index.vue'), meta: { titleKey: 'navigation.settings', icon: 'setting', groupKey: 'navigation.manage', order: 10 } }
  ]
} satisfies RouteModule
