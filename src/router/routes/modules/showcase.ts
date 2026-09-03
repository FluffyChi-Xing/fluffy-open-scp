import ChartsPage from '@/pages/showcase/ChartsPage.vue'
import ComponentsPage from '@/pages/showcase/ComponentsPage.vue'
import FeedbackPage from '@/pages/showcase/FeedbackPage.vue'
import FormPage from '@/pages/showcase/FormPage.vue'
import IconsPage from '@/pages/showcase/IconsPage.vue'
import ResultPage from '@/pages/showcase/ResultPage.vue'
import TablePage from '@/pages/showcase/TablePage.vue'
import TreePage from '@/pages/showcase/TreePage.vue'
import TokensPage from '@/pages/showcase/TokensPage.vue'
import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'showcase-components', path: 'showcase/components', component: ComponentsPage, meta: { titleKey: 'navigation.components', icon: 'components', groupKey: 'navigation.showcase', order: 10 } },
    { name: 'showcase-forms', path: 'showcase/forms', component: FormPage, meta: { titleKey: 'navigation.forms', icon: 'form', groupKey: 'navigation.showcase', order: 20 } },
    { name: 'showcase-table', path: 'showcase/table', component: TablePage, meta: { titleKey: 'navigation.table', icon: 'table', groupKey: 'navigation.showcase', order: 30 } },
    { name: 'showcase-charts', path: 'showcase/charts', component: ChartsPage, meta: { titleKey: 'navigation.charts', icon: 'chart', groupKey: 'navigation.showcase', order: 40 } },
    { name: 'showcase-feedback', path: 'showcase/feedback', component: FeedbackPage, meta: { titleKey: 'navigation.feedback', icon: 'feedback', groupKey: 'navigation.showcase', order: 50 } },
    { name: 'showcase-results', path: 'showcase/results', component: ResultPage, meta: { titleKey: 'navigation.results', icon: 'result', groupKey: 'navigation.showcase', order: 60 } },
    { name: 'showcase-tokens', path: 'showcase/tokens', component: TokensPage, meta: { titleKey: 'navigation.tokens', icon: 'tokens', groupKey: 'navigation.showcase', order: 70 } },
    { name: 'showcase-icons', path: 'showcase/icons', component: IconsPage, meta: { titleKey: 'navigation.icons', icon: 'icons', groupKey: 'navigation.showcase', order: 80 } },
    { name: 'showcase-tree', path: 'showcase/tree', component: TreePage, meta: { titleKey: 'navigation.tree', icon: 'project', groupKey: 'navigation.showcase', order: 90 } }
  ]
} satisfies RouteModule
