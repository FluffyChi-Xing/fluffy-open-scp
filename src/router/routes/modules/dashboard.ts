import HomePage from '@/pages/HomePage.vue'
import ProjectsPage from '@/pages/ProjectsPage.vue'
import DeploymentsPage from '@/pages/DeploymentsPage.vue'
import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'overview', path: '', component: HomePage, meta: { titleKey: 'navigation.home', icon: 'dashboard', groupKey: 'navigation.workspace', order: 10, noAffix: true } },
    { name: 'projects', path: 'projects', component: ProjectsPage, meta: { titleKey: 'navigation.projects', icon: 'project', groupKey: 'navigation.workspace', order: 20 } },
    { name: 'deployments', path: 'deployments', component: DeploymentsPage, meta: { titleKey: 'navigation.deployments', icon: 'deployment', groupKey: 'navigation.workspace', order: 30 } }
  ]
} satisfies RouteModule
