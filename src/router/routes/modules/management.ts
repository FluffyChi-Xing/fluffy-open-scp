import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'runtime-observer', path: 'runtime-observer', component: () => import('@/pages/runtime-observer/index.vue'), meta: { titleKey: 'navigation.runtimeObserver', icon: 'activity', groupKey: 'navigation.manage', order: 5 } },
    { name: 'settings', path: 'settings', component: () => import('@/pages/settings/index.vue'), meta: { titleKey: 'navigation.settings', icon: 'setting', groupKey: 'navigation.manage', order: 10 } }
  ]
} satisfies RouteModule
