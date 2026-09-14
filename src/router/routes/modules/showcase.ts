import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'showcase-components', path: 'showcase/components', component: () => import('@/pages/showcase/components/index.vue'), meta: { titleKey: 'navigation.components', icon: 'components', groupKey: 'navigation.showcase', hideInMenu: true, order: 10 } },
    { name: 'showcase-forms', path: 'showcase/forms', component: () => import('@/pages/showcase/forms/index.vue'), meta: { titleKey: 'navigation.forms', icon: 'form', groupKey: 'navigation.showcase', hideInMenu: true, order: 20 } },
    { name: 'showcase-table', path: 'showcase/table', component: () => import('@/pages/showcase/table/index.vue'), meta: { titleKey: 'navigation.table', icon: 'table', groupKey: 'navigation.showcase', hideInMenu: true, order: 30 } },
    { name: 'showcase-charts', path: 'showcase/charts', component: () => import('@/pages/showcase/charts/index.vue'), meta: { titleKey: 'navigation.charts', icon: 'chart', groupKey: 'navigation.showcase', hideInMenu: true, order: 40 } },
    { name: 'showcase-feedback', path: 'showcase/feedback', component: () => import('@/pages/showcase/feedback/index.vue'), meta: { titleKey: 'navigation.feedback', icon: 'feedback', groupKey: 'navigation.showcase', hideInMenu: true, order: 50 } },
    { name: 'showcase-results', path: 'showcase/results', component: () => import('@/pages/showcase/results/index.vue'), meta: { titleKey: 'navigation.results', icon: 'result', groupKey: 'navigation.showcase', hideInMenu: true, order: 60 } },
    { name: 'showcase-tokens', path: 'showcase/tokens', component: () => import('@/pages/showcase/tokens/index.vue'), meta: { titleKey: 'navigation.tokens', icon: 'tokens', groupKey: 'navigation.showcase', hideInMenu: true, order: 70 } },
    { name: 'showcase-icons', path: 'showcase/icons', component: () => import('@/pages/showcase/icons/index.vue'), meta: { titleKey: 'navigation.icons', icon: 'icons', groupKey: 'navigation.showcase', hideInMenu: true, order: 80 } },
    { name: 'showcase-tree', path: 'showcase/tree', component: () => import('@/pages/showcase/tree/index.vue'), meta: { titleKey: 'navigation.tree', icon: 'project', groupKey: 'navigation.showcase', hideInMenu: true, order: 90 } }
  ]
} satisfies RouteModule
