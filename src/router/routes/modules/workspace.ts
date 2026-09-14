import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'overview', path: '', component: () => import('@/pages/overview/index.vue'), meta: { titleKey: 'navigation.home', icon: 'dashboard', order: 0, noAffix: true } },
    { name: 'workspace', path: 'workspace', component: () => import('@/pages/workspace/index.vue'), meta: { titleKey: 'navigation.workspaceHome', icon: 'project', groupKey: 'navigation.workspace', order: 10 } },
  {
    name: 'notes',
    path: 'notes',
    component: () => import('@/pages/workspace/notes.vue'),
    meta: {
      titleKey: 'navigation.notes',
      icon: 'sticky',
      groupKey: 'navigation.workspace',
      order: 25,
    },
  },
  ],
} satisfies RouteModule
