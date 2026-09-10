import OverviewPage from '@/pages/overview/index.vue'
import WorkspacePage from '@/pages/workspace/index.vue'
import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'overview', path: '', component: OverviewPage, meta: { titleKey: 'navigation.home', icon: 'dashboard', order: 0, noAffix: true } },
    { name: 'workspace', path: 'workspace', component: WorkspacePage, meta: { titleKey: 'navigation.workspaceHome', icon: 'project', groupKey: 'navigation.workspace', order: 10 } },
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
