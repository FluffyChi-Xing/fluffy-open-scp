import ChartsPage from '@/pages/showcase/charts/index.vue'
import ComponentsPage from '@/pages/showcase/components/index.vue'
import FeedbackPage from '@/pages/showcase/feedback/index.vue'
import FormPage from '@/pages/showcase/forms/index.vue'
import IconsPage from '@/pages/showcase/icons/index.vue'
import ResultPage from '@/pages/showcase/results/index.vue'
import TablePage from '@/pages/showcase/table/index.vue'
import TreePage from '@/pages/showcase/tree/index.vue'
import TokensPage from '@/pages/showcase/tokens/index.vue'
import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'showcase-components', path: 'showcase/components', component: ComponentsPage, meta: { titleKey: 'navigation.components', icon: 'components', groupKey: 'navigation.showcase', hideInMenu: true, order: 10 } },
    { name: 'showcase-forms', path: 'showcase/forms', component: FormPage, meta: { titleKey: 'navigation.forms', icon: 'form', groupKey: 'navigation.showcase', hideInMenu: true, order: 20 } },
    { name: 'showcase-table', path: 'showcase/table', component: TablePage, meta: { titleKey: 'navigation.table', icon: 'table', groupKey: 'navigation.showcase', hideInMenu: true, order: 30 } },
    { name: 'showcase-charts', path: 'showcase/charts', component: ChartsPage, meta: { titleKey: 'navigation.charts', icon: 'chart', groupKey: 'navigation.showcase', hideInMenu: true, order: 40 } },
    { name: 'showcase-feedback', path: 'showcase/feedback', component: FeedbackPage, meta: { titleKey: 'navigation.feedback', icon: 'feedback', groupKey: 'navigation.showcase', hideInMenu: true, order: 50 } },
    { name: 'showcase-results', path: 'showcase/results', component: ResultPage, meta: { titleKey: 'navigation.results', icon: 'result', groupKey: 'navigation.showcase', hideInMenu: true, order: 60 } },
    { name: 'showcase-tokens', path: 'showcase/tokens', component: TokensPage, meta: { titleKey: 'navigation.tokens', icon: 'tokens', groupKey: 'navigation.showcase', hideInMenu: true, order: 70 } },
    { name: 'showcase-icons', path: 'showcase/icons', component: IconsPage, meta: { titleKey: 'navigation.icons', icon: 'icons', groupKey: 'navigation.showcase', hideInMenu: true, order: 80 } },
    { name: 'showcase-tree', path: 'showcase/tree', component: TreePage, meta: { titleKey: 'navigation.tree', icon: 'project', groupKey: 'navigation.showcase', hideInMenu: true, order: 90 } }
  ]
} satisfies RouteModule
